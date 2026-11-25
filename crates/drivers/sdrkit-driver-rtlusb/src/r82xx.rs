//! The Rafael Micro R820T, R820T2, R860 and R828D tuners (specification section 4.1), and the tuning rules of the
//! RTL-SDR Blog V4 and V4L boards (section 5.4). The tuner is low-IF: it mixes high-side to an IF of 1.7 to 4.57 MHz.
//!
//! The tuner cannot be read back register by register, so the driver keeps an image of the writable registers and
//! skips a write whose bytes equal the image.

use crate::chip::Chip;
use crate::RtlError;

/// The 8-bit I2C address of the R820T family.
pub const R820T_ADDR: u8 = 0x34;
/// The 8-bit I2C address of the R828D.
pub const R828D_ADDR: u8 = 0x74;

/// The most data bytes in one I2C write.
const MAX_WRITE: usize = 7;
/// The first register of the image.
const FIRST: u8 = 0x05;
/// The register values of the initialisation, 0x05 to 0x22.
const INIT: [u8; 30] = [
    0x83, 0x32, 0x75, 0xC0, 0x40, 0xD6, 0x6C, 0xF5, 0x63, 0x75, 0x68, 0x6C, 0x83, 0x80, 0x00, 0x0F,
    0x00, 0xC0, 0x30, 0x48, 0xCC, 0x60, 0x00, 0x54, 0xAE, 0x4A, 0xC0, 0x00, 0x00, 0x00,
];
/// The VCO range in kHz.
const VCO_MIN_KHZ: u64 = 1_770_000;
const VCO_MAX_KHZ: u64 = 3_540_000;
/// The IF after the initialisation.
pub const INIT_IF_HZ: u32 = 3_570_000;
/// The LO of the filter calibration.
const CAL_LO_HZ: u64 = 56_000_000;
/// The frequency of the upconverter LO of a Blog V4 or V4L.
const UPCONVERTER_HZ: u64 = 28_800_000;

/// The board around the tuner, from the USB strings.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Board {
    /// Any dongle that is not an RTL-SDR Blog V4 or V4L.
    Generic,
    /// RTL-SDR Blog V4: an R828D with an upconverter, a triplexer and notch filters.
    BlogV4,
    /// RTL-SDR Blog V4L: an R820T-family tuner with an upconverter.
    BlogV4L,
}

/// Which chip of the family.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Chip82 {
    /// R820T, R820T2 or R860 at 0x34.
    R820T,
    /// R828D at 0x74.
    R828D,
}

/// The input band of a Blog V4 or V4L.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Band {
    Hf,
    Vhf,
    Uhf,
}

/// The gain steps of the LNA and of the mixer in tenth dB, index 0 to 15.
const LNA_STEPS: [i32; 16] = [0, 9, 13, 40, 38, 13, 31, 22, 26, 31, 26, 14, 19, 5, 35, 13];
const MIXER_STEPS: [i32; 16] = [0, 5, 10, 10, 19, 9, 10, 25, 17, 10, 8, 16, 13, 6, 3, -8];

/// The gains that the tuner offers, in tenth dB.
pub const GAINS: [i32; 29] = [
    0, 9, 14, 27, 37, 77, 87, 125, 144, 157, 166, 197, 207, 229, 254, 280, 297, 328, 338, 364, 372,
    386, 402, 421, 434, 439, 445, 480, 496,
];

/// The LNA index, the mixer index and the total gain in tenth dB for a manual gain request of `g` tenth dB: the first
/// row of the ladder whose total is at least `g`, or the last row for a request above every row.
pub fn gain_ladder(g: i32) -> (u8, u8, i32) {
    let (mut total, mut last) = (0, (0u8, 0u8, 0i32));
    if g <= 0 {
        return last;
    }
    for i in 1..16u8 {
        total += LNA_STEPS[usize::from(i)];
        last = (i, i - 1, total);
        if total >= g {
            return last;
        }
        total += MIXER_STEPS[usize::from(i)];
        last = (i, i, total);
        if total >= g {
            return last;
        }
    }
    last
}

/// The narrow low-pass widths of the IF filter, index 0 to 9.
const LOW_PASS: [u32; 10] = [
    1_700_000, 1_600_000, 1_550_000, 1_450_000, 1_200_000, 900_000, 700_000, 550_000, 450_000,
    350_000,
];

/// The IF filter set-up for a bandwidth: R0A bit 4, R0B (mask 0xEF), the IF and the filter width.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Filter {
    /// R0A, mask 0x10.
    pub r0a: u8,
    /// R0B, mask 0xEF.
    pub r0b: u8,
    /// The IF in Hz.
    pub if_hz: u32,
    /// The width of the filter in Hz.
    pub width_hz: u32,
}

/// The IF filter set-up for the bandwidth `bw` in Hz.
pub fn filter_for(bw: u32) -> Filter {
    if bw > 7_000_000 {
        return Filter {
            r0a: 0x10,
            r0b: 0x0B,
            if_hz: 4_570_000,
            width_hz: 8_000_000,
        };
    }
    if bw > 6_000_000 {
        return Filter {
            r0a: 0x10,
            r0b: 0x2A,
            if_hz: 4_570_000,
            width_hz: 7_000_000,
        };
    }
    if bw > 2_430_000 {
        return Filter {
            r0a: 0x10,
            r0b: 0x6B,
            if_hz: 3_570_000,
            width_hz: 6_000_000,
        };
    }
    let (base, h) = match bw {
        0..=1_700_000 => (0xE0u8, 0u32),
        1_700_001..=2_050_000 => (0xA0, 350_000),
        2_050_001..=2_080_000 => (0xC0, 380_000),
        _ => (0x80, 730_000),
    };
    let b = bw.saturating_sub(h);
    let idx = LOW_PASS.iter().rposition(|&w| w >= b).unwrap_or(0);
    let width = h + LOW_PASS[idx];
    Filter {
        r0a: 0x00,
        r0b: base | (15 - idx as u8),
        if_hz: 2_300_000 + h - width / 2,
        width_hz: width,
    }
}

/// The values that the PLL writes for a local oscillator frequency.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pll {
    /// The mixer divider D, a power of two from 2 to 64.
    pub divider: u32,
    /// The SEL_DIV code before the fine-tune correction.
    pub sel_div: u8,
    /// The integer part of the PLL.
    pub nint: u32,
    /// The fraction of the PLL in 1/65536.
    pub sdm: u16,
    /// R14.
    pub r14: u8,
}

/// The PLL values for the LO `lo_hz` with the reference `ref_hz` and the reference value `p` (2 for the R820T family,
/// 1 for the R828D).
///
/// # Errors
///
/// [`RtlError::NoPllSetting`] if no divider puts the VCO in range or the integer part is out of range.
pub fn pll_for(lo_hz: u64, ref_hz: u32, p: u32) -> Result<Pll, RtlError> {
    let no = RtlError::NoPllSetting { hz: lo_hz };
    let lo_khz = (lo_hz + 500) / 1000;
    let (divider, sel_div) = (0..6u8)
        .map(|n| (2u64 << n, n))
        .find(|&(d, _)| {
            let v = lo_khz * d;
            (VCO_MIN_KHZ..VCO_MAX_KHZ).contains(&v)
        })
        .ok_or(no.clone())?;
    let vco = lo_hz * divider;
    let r = u64::from(ref_hz);
    let v = (r + 65536 * vco) / (2 * r);
    let nint = (v >> 16) as u32;
    let sdm = (v & 0xFFFF) as u16;
    if nint > 128 / p - 1 || nint < 13 {
        return Err(no);
    }
    let ni = (nint - 13) / 4;
    let si = nint - 4 * ni - 13;
    Ok(Pll {
        divider: divider as u32,
        sel_div,
        nint,
        sdm,
        r14: (ni + (si << 6)) as u8,
    })
}

/// One row of the band table: the LO in whole MHz from which it applies, the open-drain bit, R1A and R1B.
const BANDS: [(u32, u8, u8, u8); 21] = [
    (0, 0x08, 0x02, 0xDF),
    (50, 0x08, 0x02, 0xBE),
    (55, 0x08, 0x02, 0x8B),
    (60, 0x08, 0x02, 0x7B),
    (65, 0x08, 0x02, 0x69),
    (70, 0x08, 0x02, 0x58),
    (75, 0x00, 0x02, 0x44),
    (80, 0x00, 0x02, 0x44),
    (90, 0x00, 0x02, 0x34),
    (100, 0x00, 0x02, 0x34),
    (110, 0x00, 0x02, 0x24),
    (120, 0x00, 0x02, 0x24),
    (140, 0x00, 0x02, 0x14),
    (180, 0x00, 0x02, 0x13),
    (220, 0x00, 0x02, 0x13),
    (250, 0x00, 0x02, 0x11),
    (280, 0x00, 0x02, 0x00),
    (310, 0x00, 0x41, 0x00),
    (450, 0x00, 0x41, 0x00),
    (588, 0x00, 0x40, 0x00),
    (650, 0x00, 0x40, 0x00),
];

/// Reverses the bit order of a byte read from the tuner.
fn reverse(b: u8) -> u8 {
    b.reverse_bits()
}

/// An R820T-family or R828D tuner.
#[derive(Clone, Debug)]
pub struct R82xx {
    addr: u8,
    chip: Chip82,
    board: Board,
    /// The nominal reference of the PLL.
    xtal: u32,
    /// The reference after the frequency correction.
    ref_hz: u32,
    /// The image of registers 0x00 to 0x22.
    regs: [u8; 0x23],
    if_hz: u32,
    initialised: bool,
    locked: bool,
    /// The band of the last tune of a V4 or V4L; `None` after the initialisation.
    band: Option<Band>,
    /// The cable input of a generic R828D is selected; `None` after the initialisation.
    cable: Option<bool>,
}

impl R82xx {
    /// A tuner of `chip` on `board`, whose PLL reference is `xtal` Hz.
    pub fn new(chip: Chip82, board: Board, xtal: u32) -> R82xx {
        R82xx {
            addr: match chip {
                Chip82::R820T => R820T_ADDR,
                Chip82::R828D => R828D_ADDR,
            },
            chip,
            board,
            xtal,
            ref_hz: xtal,
            regs: [0; 0x23],
            if_hz: INIT_IF_HZ,
            initialised: false,
            locked: false,
            band: None,
            cable: None,
        }
    }

    /// The chip.
    pub fn chip(&self) -> Chip82 {
        self.chip
    }

    /// The IF in Hz.
    pub fn if_hz(&self) -> u32 {
        self.if_hz
    }

    /// The last PLL lock state.
    pub fn locked(&self) -> bool {
        self.locked
    }

    /// The image of registers 0x05 to 0x1F.
    pub fn image(&self) -> [u8; 27] {
        let mut out = [0u8; 27];
        out.copy_from_slice(&self.regs[0x05..0x20]);
        out
    }

    /// Sets the frequency correction of the reference: `ppm` parts per million.
    pub fn set_ppm(&mut self, ppm: f64) {
        self.ref_hz = corrected(self.xtal, ppm);
    }

    /// The reference value p of the PLL fine-tune check.
    fn p(&self) -> u8 {
        if self.chip == Chip82::R828D || self.board == Board::BlogV4L {
            1
        } else {
            2
        }
    }

    /// Writes `data` from register `reg`, in messages of at most 7 data bytes. Unless `force`, a write whose bytes
    /// equal the image is skipped.
    fn write(&mut self, c: &mut Chip, reg: u8, data: &[u8], force: bool) -> Result<(), RtlError> {
        let at = usize::from(reg);
        if !force && self.regs[at..at + data.len()] == *data {
            return Ok(());
        }
        let mut msg = [0u8; MAX_WRITE + 1];
        for (k, part) in data.chunks(MAX_WRITE).enumerate() {
            msg[0] = reg + (k * MAX_WRITE) as u8;
            msg[1..=part.len()].copy_from_slice(part);
            c.i2c_write(self.addr, &msg[..=part.len()])?;
        }
        self.regs[at..at + data.len()].copy_from_slice(data);
        Ok(())
    }

    /// Writes register `reg` with the bits of `mask` taken from `value` and the others from the image.
    fn mask(&mut self, c: &mut Chip, reg: u8, value: u8, mask: u8) -> Result<(), RtlError> {
        let v = (self.regs[usize::from(reg)] & !mask) | (value & mask);
        self.write(c, reg, &[v], false)
    }

    /// Reads registers 0 to `n - 1` into `out`, in register bit order.
    fn read(&mut self, c: &mut Chip, out: &mut [u8]) -> Result<(), RtlError> {
        c.i2c_write(self.addr, &[0x00])?;
        c.i2c_read(self.addr, out)?;
        for b in out.iter_mut() {
            *b = reverse(*b);
        }
        Ok(())
    }

    /// Initialises the tuner: the register table, the channel set-up with the filter calibration, and the detector
    /// set-up. The IF is then 3.57 MHz. The gain mode must be set after it.
    ///
    /// # Errors
    ///
    /// [`RtlError::PllNotLocked`] if the PLL of the calibration does not lock, else the error of a transfer.
    pub fn init(&mut self, c: &mut Chip) -> Result<(), RtlError> {
        self.write(c, FIRST, &INIT, true)?;
        // Channel set-up for a 6 MHz channel.
        self.mask(c, 0x0C, 0x00, 0x0F)?;
        self.mask(c, 0x13, 0x31, 0x3F)?;
        self.mask(c, 0x1D, 0x00, 0x38)?;
        let code = self.calibrate(c)?;
        self.mask(c, 0x0A, 0x10 | code, 0x1F)?;
        self.mask(c, 0x0B, 0x6B, 0xEF)?;
        self.mask(c, 0x07, 0x00, 0x80)?;
        self.mask(c, 0x06, 0x10, 0x30)?;
        self.mask(c, 0x1E, 0x60, 0x60)?;
        self.mask(c, 0x05, 0x00, 0x80)?;
        self.mask(c, 0x1F, 0x00, 0x80)?;
        self.mask(c, 0x0F, 0x00, 0x80)?;
        self.mask(c, 0x19, 0x60, 0x60)?;
        // Detector and AGC set-up.
        self.mask(c, 0x1D, 0xE5, 0xC7)?;
        self.mask(c, 0x1C, 0x24, 0xF8)?;
        self.write(c, 0x0D, &[0x53], false)?;
        self.write(c, 0x0E, &[0x75], false)?;
        self.mask(c, 0x05, 0x00, 0x60)?;
        self.mask(c, 0x06, 0x00, 0x08)?;
        self.mask(c, 0x11, 0x38, 0x38)?;
        self.mask(c, 0x17, 0x30, 0x30)?;
        self.mask(c, 0x0A, 0x40, 0x60)?;
        self.mask(c, 0x1D, 0x00, 0x38)?;
        self.mask(c, 0x1C, 0x00, 0x04)?;
        self.mask(c, 0x06, 0x00, 0x40)?;
        self.mask(c, 0x1A, 0x30, 0x30)?;
        self.mask(c, 0x1D, 0x18, 0x38)?;
        self.mask(c, 0x1C, 0x04, 0x04)?;
        self.mask(c, 0x1E, 0x0E, 0x1F)?;
        self.mask(c, 0x1A, 0x20, 0x30)?;
        self.if_hz = INIT_IF_HZ;
        self.initialised = true;
        self.band = None;
        self.cable = None;
        Ok(())
    }

    /// The filter calibration: at most two attempts; returns the calibration code (0 if both read 0xF).
    fn calibrate(&mut self, c: &mut Chip) -> Result<u8, RtlError> {
        let mut code = 0;
        for _ in 0..2 {
            self.mask(c, 0x0B, 0x6B, 0x60)?;
            self.mask(c, 0x0F, 0x04, 0x04)?;
            self.mask(c, 0x10, 0x00, 0x03)?;
            self.pll(c, CAL_LO_HZ, &mut |_| {})?;
            self.mask(c, 0x0B, 0x10, 0x10)?;
            self.mask(c, 0x0B, 0x00, 0x10)?;
            self.mask(c, 0x0F, 0x00, 0x04)?;
            let mut r = [0u8; 5];
            self.read(c, &mut r)?;
            code = r[4] & 0x0F;
            if code != 0x0 && code != 0xF {
                return Ok(code);
            }
        }
        Ok(if code == 0xF { 0 } else { code })
    }

    /// Programs the PLL to `lo_hz` and checks the lock.
    ///
    /// # Errors
    ///
    /// [`RtlError::NoPllSetting`] or [`RtlError::PllNotLocked`], else the error of a transfer.
    /// `edge` is called with `true` just before the LO changes and with `false` after the lock check.
    fn pll(
        &mut self,
        c: &mut Chip,
        lo_hz: u64,
        edge: &mut dyn FnMut(bool),
    ) -> Result<(), RtlError> {
        self.mask(c, 0x10, 0x00, 0x10)?;
        self.mask(c, 0x1A, 0x00, 0x0C)?;
        self.mask(c, 0x12, 0x80, 0xE0)?;
        let p = pll_for(lo_hz, self.ref_hz, u32::from(self.p()))?;
        let mut r = [0u8; 5];
        self.read(c, &mut r)?;
        let t = (r[4] >> 4) & 0x03;
        let mut n = p.sel_div;
        if t > self.p() {
            n = n.saturating_sub(1);
        } else if t < self.p() {
            n = (n + 1).min(7);
        }
        let mut regs = [0u8; 7];
        regs.copy_from_slice(&self.regs[0x10..0x17]);
        regs[0] = (regs[0] & !0xE0) | (n << 5);
        regs[4] = p.r14;
        regs[2] = (regs[2] & !0x08) | if p.sdm == 0 { 0x08 } else { 0x00 };
        regs[5] = (p.sdm & 0xFF) as u8;
        regs[6] = (p.sdm >> 8) as u8;
        edge(true);
        let r = self.program(c, &regs);
        edge(false);
        r?;
        if !self.locked {
            return Err(RtlError::PllNotLocked { hz: lo_hz });
        }
        self.mask(c, 0x1A, 0x08, 0x08)
    }

    /// Writes R10 to R16 and checks the lock, a second time with more VCO current.
    fn program(&mut self, c: &mut Chip, regs: &[u8; 7]) -> Result<(), RtlError> {
        self.write(c, 0x10, regs, false)?;
        self.locked = self.lock_check(c)?;
        if !self.locked {
            self.mask(c, 0x12, 0x60, 0xE0)?;
            self.locked = self.lock_check(c)?;
        }
        Ok(())
    }

    fn lock_check(&mut self, c: &mut Chip) -> Result<bool, RtlError> {
        let mut r = [0u8; 3];
        self.read(c, &mut r)?;
        Ok(r[2] & 0x40 != 0)
    }

    /// Sets the IF filter for the bandwidth `bw` in Hz and returns the new IF. The demodulator IF and the tune must
    /// follow, because the LO depends on the IF.
    ///
    /// # Errors
    ///
    /// The error of a transfer.
    pub fn set_bandwidth(&mut self, c: &mut Chip, bw: u32) -> Result<Filter, RtlError> {
        let f = filter_for(bw);
        self.mask(c, 0x0A, f.r0a, 0x10)?;
        self.mask(c, 0x0B, f.r0b, 0xEF)?;
        self.if_hz = f.if_hz;
        Ok(f)
    }

    /// Tunes to the RF frequency `rf_hz`: the upconverter rule of a Blog board, the band filter, the PLL at RF plus IF,
    /// and the input selection. `edge` is called with `true` just before the LO changes and with `false` after the
    /// lock check.
    ///
    /// # Errors
    ///
    /// [`RtlError::NoPllSetting`] or [`RtlError::PllNotLocked`], else the error of a transfer.
    pub fn set_frequency(
        &mut self,
        c: &mut Chip,
        rf_hz: u64,
        edge: &mut dyn FnMut(bool),
    ) -> Result<(), RtlError> {
        let blog = self.board != Board::Generic;
        let tuned = if blog && rf_hz < UPCONVERTER_HZ {
            rf_hz + UPCONVERTER_HZ
        } else {
            rf_hz
        };
        let lo = tuned + u64::from(self.if_hz);
        self.band_filter(c, lo)?;
        self.pll(c, lo, edge)?;
        match self.board {
            Board::Generic if self.chip == Chip82::R828D => self.select_input(c, rf_hz),
            Board::Generic => Ok(()),
            _ => self.blog_rules(c, rf_hz),
        }
    }

    /// The band filter set-up for the LO `lo_hz`.
    fn band_filter(&mut self, c: &mut Chip, lo_hz: u64) -> Result<(), RtlError> {
        let mhz = (lo_hz / 1_000_000) as u32;
        let row = BANDS
            .iter()
            .rev()
            .find(|r| r.0 <= mhz)
            .copied()
            .unwrap_or(BANDS[0]);
        self.mask(c, 0x17, row.1, 0x08)?;
        self.mask(c, 0x1A, row.2, 0xC3)?;
        self.write(c, 0x1B, &[row.3], false)?;
        // Crystal load: high drive, 0 pF.
        self.mask(c, 0x10, 0x00, 0x0B)?;
        self.mask(c, 0x08, 0x00, 0x3F)?;
        self.mask(c, 0x09, 0x00, 0x3F)
    }

    /// The air input above 345 MHz and the cable input below, on a generic R828D board.
    fn select_input(&mut self, c: &mut Chip, rf_hz: u64) -> Result<(), RtlError> {
        let cable = rf_hz <= 345_000_000;
        if self.cable != Some(cable) {
            self.mask(c, 0x05, if cable { 0x60 } else { 0x00 }, 0x60)?;
            self.cable = Some(cable);
        }
        Ok(())
    }

    /// The notch filters, the HF band set-up and the input switching of a Blog V4 or V4L.
    fn blog_rules(&mut self, c: &mut Chip, rf_hz: u64) -> Result<(), RtlError> {
        let v4 = self.board == Board::BlogV4;
        let mhz = rf_hz as f64 / 1e6;
        if v4 {
            let open =
                mhz <= 2.2 || (85.0..=112.0).contains(&mhz) || (172.0..=242.0).contains(&mhz);
            self.mask(c, 0x17, if open { 0x00 } else { 0x08 }, 0x08)?;
        }
        let band = if rf_hz <= UPCONVERTER_HZ {
            Band::Hf
        } else if v4 && rf_hz < 250_000_000 {
            Band::Vhf
        } else {
            Band::Uhf
        };
        if band == Band::Hf {
            self.mask(c, 0x1A, 0x40, 0xC3)?;
            self.write(c, 0x1B, &[0x00], false)?;
        }
        if self.band != Some(band) {
            if v4 {
                self.mask(c, 0x06, if band == Band::Hf { 0x08 } else { 0x00 }, 0x08)?;
            }
            c.gpio_output(5)?;
            c.gpio_set(5, band != Band::Hf)?;
            let cable1 = if v4 {
                band == Band::Vhf
            } else {
                band == Band::Hf
            };
            self.mask(c, 0x05, if cable1 { 0x40 } else { 0x00 }, 0x40)?;
            self.mask(c, 0x05, if band == Band::Uhf { 0x00 } else { 0x20 }, 0x20)?;
            self.band = Some(band);
        }
        Ok(())
    }

    /// Manual gain mode with the gain `g` in tenth dB from the ladder; returns the gain set.
    ///
    /// # Errors
    ///
    /// The error of a transfer.
    pub fn set_manual_gain(&mut self, c: &mut Chip, g: i32) -> Result<i32, RtlError> {
        self.mask(c, 0x05, 0x10, 0x10)?;
        self.mask(c, 0x07, 0x00, 0x10)?;
        let mut r = [0u8; 4];
        self.read(c, &mut r)?;
        self.mask(c, 0x0C, 0x08, 0x9F)?;
        let (lna, mix, total) = gain_ladder(g);
        self.mask(c, 0x05, lna, 0x0F)?;
        self.mask(c, 0x07, mix, 0x0F)?;
        Ok(total)
    }

    /// Automatic gain mode: the LNA and the mixer follow the detectors of the tuner, the VGA is at code 11.
    ///
    /// # Errors
    ///
    /// The error of a transfer.
    pub fn set_auto_gain(&mut self, c: &mut Chip) -> Result<(), RtlError> {
        self.mask(c, 0x05, 0x00, 0x10)?;
        self.mask(c, 0x07, 0x10, 0x10)?;
        self.mask(c, 0x0C, 0x0B, 0x9F)
    }

    /// Puts the tuner in standby. The next use needs [`R82xx::init`]. Nothing is written if it was never initialised.
    ///
    /// # Errors
    ///
    /// The error of a transfer.
    pub fn standby(&mut self, c: &mut Chip) -> Result<(), RtlError> {
        if !self.initialised {
            return Ok(());
        }
        const STANDBY: [(u8, u8); 11] = [
            (0x06, 0xB1),
            (0x05, 0xA0),
            (0x07, 0x3A),
            (0x08, 0x40),
            (0x09, 0xC0),
            (0x0A, 0x36),
            (0x0C, 0x35),
            (0x0F, 0x68),
            (0x11, 0x03),
            (0x17, 0xF4),
            (0x19, 0x0C),
        ];
        for (reg, v) in STANDBY {
            self.write(c, reg, &[v], true)?;
        }
        self.initialised = false;
        Ok(())
    }

    /// The tuner was initialised and is not in standby.
    pub fn initialised(&self) -> bool {
        self.initialised
    }
}

/// The RF range in Hz of a tuner on `board`: from the lowest LO with the IF below it (from 0.5 MHz through the
/// upconverter of a Blog board) to the highest LO.
pub fn range_hz(board: Board) -> (f64, f64) {
    let min = if board == Board::Generic { 24e6 } else { 0.5e6 };
    (min, 1766e6)
}

/// `hz` corrected by `ppm` parts per million, truncated to whole Hz.
pub fn corrected(hz: u32, ppm: f64) -> u32 {
    (f64::from(hz) * (1.0 + ppm / 1e6)).trunc() as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_gain_ladder_takes_the_first_row_at_or_above() {
        assert_eq!(gain_ladder(0), (0, 0, 0));
        assert_eq!(gain_ladder(297), (8, 8, 297));
        assert_eq!(gain_ladder(290), (8, 8, 297));
        assert_eq!(gain_ladder(481), (14, 14, 483));
        assert_eq!(gain_ladder(484), (15, 14, 496));
        assert_eq!(gain_ladder(496), (15, 14, 496));
        assert_eq!(gain_ladder(497), (15, 15, 488));
        for g in GAINS {
            assert_eq!(gain_ladder(g).2, g, "the advertised gain {g} is a row");
        }
    }

    #[test]
    fn the_filter_table_matches_the_common_widths() {
        let rows = [
            (250_000, 0x00, 0xE6, 2_125_000, 350_000),
            (300_000, 0x00, 0xE6, 2_125_000, 350_000),
            (500_000, 0x00, 0xE8, 2_025_000, 550_000),
            (1_000_000, 0x00, 0xEB, 1_700_000, 1_200_000),
            (1_024_000, 0x00, 0xEB, 1_700_000, 1_200_000),
            (1_200_000, 0x00, 0xEB, 1_700_000, 1_200_000),
            (1_400_000, 0x00, 0xEC, 1_575_000, 1_450_000),
            (1_500_000, 0x00, 0xED, 1_525_000, 1_550_000),
            (1_800_000, 0x00, 0xAC, 1_750_000, 1_800_000),
            (1_920_000, 0x00, 0xAE, 1_675_000, 1_950_000),
            (2_048_000, 0x00, 0xAF, 1_625_000, 2_050_000),
            (2_400_000, 0x00, 0x8F, 1_815_000, 2_430_000),
            (2_560_000, 0x10, 0x6B, 3_570_000, 6_000_000),
            (6_000_000, 0x10, 0x6B, 3_570_000, 6_000_000),
            (7_000_000, 0x10, 0x2A, 4_570_000, 7_000_000),
            (8_000_000, 0x10, 0x0B, 4_570_000, 8_000_000),
        ];
        for (bw, r0a, r0b, if_hz, width) in rows {
            let f = filter_for(bw);
            assert_eq!((f.r0a, f.r0b, f.if_hz), (r0a, r0b, if_hz), "bw {bw}");
            assert_eq!(f.width_hz, width, "bw {bw}");
        }
    }

    #[test]
    fn pll_values_for_a_table_of_frequencies() {
        // (LO, divider, SEL_DIV, Nint, SDM, R14), reference 28.8 MHz, p = 2.
        let rows: [(u64, u32, u8, u32, u16, u8); 6] = [
            (56_000_000, 32, 4, 31, 0x1C72, 0x84),
            (101_815_000, 32, 4, 56, 0x905B, 0xCA),
            (27_660_000, 64, 5, 30, 0xBBBC, 0x44),
            (433_920_000 + 1_815_000, 8, 2, 60, 0x84CD, 0xCB),
            (1_090_000_000 + 3_570_000, 2, 0, 37, 0xF89F, 0x06),
            (1_769_000_000, 2, 0, 61, 0x6C72, 0x0C),
        ];
        for (lo, d, n, nint, sdm, r14) in rows {
            let p = pll_for(lo, 28_800_000, 2).unwrap();
            assert_eq!(
                (p.divider, p.sel_div, p.nint, p.sdm, p.r14),
                (d, n, nint, sdm, r14),
                "LO {lo}"
            );
            // The PLL frequency is within 1/65536 of the reference step of the LO.
            let lo_back = 2.0 * 28.8e6 * (f64::from(p.nint) + f64::from(p.sdm) / 65536.0)
                / f64::from(p.divider);
            assert!((lo_back - lo as f64).abs() < 28.8e6 / 65536.0, "LO {lo}");
        }
        assert!(pll_for(20_000_000, 28_800_000, 2).is_err());
        assert!(pll_for(1_771_000_000, 28_800_000, 2).is_err());
        // An R828D with a 16 MHz reference and p = 1 reaches Nint up to 127.
        let p = pll_for(1_700_000_000, 16_000_000, 1).unwrap();
        assert_eq!((p.divider, p.nint, p.sdm, p.r14), (2, 106, 0x4000, 0x57));
    }

    #[test]
    fn corrected_reference_truncates() {
        assert_eq!(corrected(28_800_000, 0.0), 28_800_000);
        assert_eq!(corrected(28_800_000, 1.0), 28_800_028);
        assert_eq!(corrected(28_800_000, -50.0), 28_798_560);
    }
}
