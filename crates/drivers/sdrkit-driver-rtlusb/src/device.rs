//! An open RTL2832U dongle: the USB and demodulator set-up, the tuner probe, and one call per operation of the
//! specification (sections 2.6, 3 and 5): sample rate, frequency, gain, frequency correction, direct sampling, offset
//! tuning, bias tee, digital AGC, the counter test mode, the endpoint reset and the close.

use crate::chip::{Chip, DEMOD_CTL, DEMOD_CTL_1, USB_EPA_CTL, USB_EPA_MAXPKT, USB_SYSCTL};
use crate::e4000::{self, E4000};
use crate::ids::{parse_eeprom, Eeprom, EEPROM_ADDR, EEPROM_LEN};
use crate::r82xx::{self, corrected, Board, Chip82, R82xx};
use crate::RtlError;
use sdrkit_driver_usb::{UsbDevice, UsbDeviceInfo};

/// The crystal of the RTL2832U.
pub const XTAL_HZ: u32 = 28_800_000;
/// The bulk IN endpoint of the samples.
pub const ENDPOINT: u8 = 0x81;
/// The packet size of the endpoint; a transfer is a whole number of packets.
pub const PACKET: usize = 512;

/// The FIR coefficients of the decimation filter, outermost tap first.
pub const FIR: [i16; 16] = [
    -54, -36, -41, -40, -32, -14, 14, 53, 101, 156, 215, 273, 327, 372, 404, 421,
];

/// The 20 bytes of page 1 registers 0x1C to 0x2F for the coefficients `fir`.
///
/// # Errors
///
/// [`RtlError::NotHere`] if a coefficient is out of its range (8-bit for 0 to 7, 12-bit for 8 to 15).
pub fn fir_bytes(fir: &[i16; 16]) -> Result<[u8; 20], RtlError> {
    let mut out = [0u8; 20];
    for (i, &c) in fir[..8].iter().enumerate() {
        if !(-128..=127).contains(&c) {
            return Err(RtlError::NotHere(
                "an 8-bit FIR coefficient is out of range",
            ));
        }
        out[i] = c as u8;
    }
    for (k, pair) in fir[8..].as_chunks::<2>().0.iter().enumerate() {
        if pair.iter().any(|c| !(-2048..=2047).contains(c)) {
            return Err(RtlError::NotHere(
                "a 12-bit FIR coefficient is out of range",
            ));
        }
        let (a, b) = ((pair[0] as u16) & 0xFFF, (pair[1] as u16) & 0xFFF);
        out[8 + 3 * k] = (a >> 4) as u8;
        out[9 + 3 * k] = (((a & 0x0F) << 4) | (b >> 8)) as u8;
        out[10 + 3 * k] = (b & 0xFF) as u8;
    }
    Ok(out)
}

/// The resampler ratio written for the rate `fs` with the crystal `xtal`, and the effective rate.
///
/// # Errors
///
/// [`RtlError::Rate`] outside 225 001 to 300 000 and 900 001 to 3 200 000.
pub fn rate_ratio(fs: u32, xtal: u32) -> Result<(u32, f64), RtlError> {
    if !(225_001..=300_000).contains(&fs) && !(900_001..=3_200_000).contains(&fs) {
        return Err(RtlError::Rate(fs));
    }
    let ratio = ((u64::from(xtal) << 22) / u64::from(fs)) as u32 & 0x0FFF_FFFC;
    let effective = ratio | ((ratio & (1 << 27)) << 1);
    Ok((ratio, f64::from(xtal) * 4_194_304.0 / f64::from(effective)))
}

/// The 14-bit sample-rate correction for `ppm` parts per million: (R3E, R3F).
pub fn ppm_correction(ppm: f64) -> (u8, u8) {
    let v = (-ppm * 16_777_216.0 / 1e6).trunc() as i32;
    (((v >> 8) & 0x3F) as u8, (v & 0xFF) as u8)
}

/// The 22-bit IF register for `if_hz` with the corrected crystal `xtal`: (R19, R1A, R1B).
pub fn if_register(if_hz: u32, xtal: u32) -> (u8, u8, u8) {
    let v = (u64::from(if_hz) << 22) / u64::from(xtal);
    let v = (v as u32).wrapping_neg() & 0x3F_FFFF;
    ((v >> 16) as u8, (v >> 8) as u8, v as u8)
}

/// The tuner chips the probe tells apart.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TunerKind {
    /// Elonics E4000.
    E4000,
    /// Fitipower FC0013.
    Fc0013,
    /// Rafael Micro R820T, R820T2 or R860.
    R820T,
    /// Rafael Micro R828D.
    R828D,
    /// FCI FC2580.
    Fc2580,
    /// Fitipower FC0012.
    Fc0012,
}

impl TunerKind {
    /// The name of the chip.
    pub fn name(self) -> &'static str {
        match self {
            TunerKind::E4000 => "E4000",
            TunerKind::Fc0013 => "FC0013",
            TunerKind::R820T => "R820T",
            TunerKind::R828D => "R828D",
            TunerKind::Fc2580 => "FC2580",
            TunerKind::Fc0012 => "FC0012",
        }
    }

    /// The driver drives this tuner.
    pub fn supported(self) -> bool {
        matches!(self, TunerKind::E4000 | TunerKind::R820T | TunerKind::R828D)
    }

    /// A zero-IF tuner (offset tuning applies).
    pub fn zero_if(self) -> bool {
        !matches!(self, TunerKind::R820T | TunerKind::R828D)
    }
}

/// Direct sampling: the antenna signal goes to one ADC input and the tuner is off.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direct {
    /// The tuner path.
    Off,
    /// ADC input 1, the I branch.
    I,
    /// ADC input 2, the Q branch.
    Q,
}

/// The gain of the tuner.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Gain {
    /// The detectors of the tuner set the gain.
    Auto,
    /// A fixed gain in tenth dB.
    Manual(i32),
}

enum Tuner {
    R82xx(R82xx),
    E4000(E4000),
}

/// Probes the tuner with the repeater on and returns the first chip that answers (specification 1.2). A failed I2C
/// transfer is "not this chip"; only an unplug is an error.
fn probe(c: &mut Chip) -> Result<Option<TunerKind>, RtlError> {
    fn reg(c: &mut Chip, addr: u8, reg: u8) -> Result<Option<u8>, RtlError> {
        match c.i2c_read_reg(addr, reg) {
            Ok(v) => Ok(Some(v)),
            Err(e) if e.unplugged() => Err(e),
            Err(_) => Ok(None),
        }
    }
    if reg(c, e4000::ADDR, 0x02)? == Some(0x40) {
        return Ok(Some(TunerKind::E4000));
    }
    if reg(c, 0xC6, 0x00)? == Some(0xA3) {
        return Ok(Some(TunerKind::Fc0013));
    }
    if reg(c, r82xx::R820T_ADDR, 0x00)? == Some(0x69) {
        return Ok(Some(TunerKind::R820T));
    }
    if reg(c, r82xx::R828D_ADDR, 0x00)? == Some(0x69) {
        return Ok(Some(TunerKind::R828D));
    }
    c.gpio_output(4)?;
    c.gpio_set(4, true)?;
    c.gpio_set(4, false)?;
    if reg(c, 0xAC, 0x01)?.map(|v| v & 0x7F) == Some(0x56) {
        return Ok(Some(TunerKind::Fc2580));
    }
    if reg(c, 0xC6, 0x00)? == Some(0xA1) {
        return Ok(Some(TunerKind::Fc0012));
    }
    Ok(None)
}

/// The board from the manufacturer and product strings.
fn board_of(manufacturer: Option<&str>, product: Option<&str>) -> Board {
    match (manufacturer, product) {
        (Some("RTLSDRBlog"), Some("Blog V4")) => Board::BlogV4,
        (Some("RTLSDRBlog"), Some("Blog V4L")) => Board::BlogV4L,
        _ => Board::Generic,
    }
}

/// The USB set-up and the demodulator open sequence, up to the tuner probe (specification 2.6 and 3.2).
fn power_up(c: &mut Chip) -> Result<(), RtlError> {
    if let Err(e) = c.usb_write(USB_SYSCTL, &[0x09]) {
        if e.unplugged() {
            return Err(e);
        }
        c.device().reset()?;
    }
    c.usb_write(USB_SYSCTL, &[0x09])?;
    c.usb_write(USB_EPA_MAXPKT, &[0x00, 0x02])?;
    c.usb_write(USB_EPA_CTL, &[0x10, 0x02])?;
    c.sys_write(DEMOD_CTL_1, 0x22)?;
    c.sys_write(DEMOD_CTL, 0xE8)?;
    c.demod_write1(1, 0x01, 0x14)?;
    c.demod_write1(1, 0x01, 0x10)?;
    c.demod_write1(1, 0x15, 0x00)?;
    c.demod_write(1, 0x16, &[0x00, 0x00])?;
    for reg in 0x16..=0x1B {
        c.demod_write1(1, reg, 0x00)?;
    }
    for (i, b) in fir_bytes(&FIR)?.into_iter().enumerate() {
        c.demod_write1(1, 0x1C + i as u8, b)?;
    }
    c.demod_write1(0, 0x19, 0x05)?;
    c.demod_write1(1, 0x93, 0xF0)?;
    c.demod_write1(1, 0x94, 0x0F)?;
    c.demod_write1(1, 0x11, 0x00)?;
    c.demod_write1(1, 0x04, 0x00)?;
    c.demod_write1(0, 0x61, 0x60)?;
    c.demod_write1(0, 0x06, 0x80)?;
    c.demod_write1(1, 0xB1, 0x1B)?;
    c.demod_write1(0, 0x0D, 0x83)
}

/// Reads the 256 bytes of the EEPROM, one byte per I2C read after a write of the start offset.
fn read_eeprom_image(c: &mut Chip) -> Result<[u8; EEPROM_LEN], RtlError> {
    let mut image = [0u8; EEPROM_LEN];
    c.i2c_write(EEPROM_ADDR, &[0x00])?;
    for b in image.iter_mut() {
        let mut one = [0u8; 1];
        c.i2c_read(EEPROM_ADDR, &mut one)?;
        *b = one[0];
    }
    Ok(image)
}

/// Opens `dev` only to read which tuner it has: the USB and demodulator set-up and the probe, then the demodulator is
/// powered down. The tuner is not initialised.
///
/// # Errors
///
/// The error of a transfer.
pub fn probe_tuner(dev: Box<dyn UsbDevice>) -> Result<Option<TunerKind>, RtlError> {
    let mut c = Chip::new(dev);
    c.device().claim(0)?;
    power_up(&mut c)?;
    let found = c.with_repeater(probe)?;
    c.sys_write(DEMOD_CTL, 0x20)?;
    Ok(found)
}

/// An open dongle with a supported tuner.
pub struct Rtl2832 {
    chip: Chip,
    kind: TunerKind,
    board: Board,
    tuner: Tuner,
    xtal: u32,
    ppm: f64,
    rate: Option<u32>,
    rate_eff: f64,
    freq: Option<u64>,
    bandwidth: u32,
    if_hz: u32,
    direct: Direct,
    offset: bool,
    gain: Gain,
    rtl_agc: bool,
    test_mode: bool,
    bias: bool,
    closed: bool,
    edge: Option<RetuneEdge>,
}

/// Called with `true` just before the frequency of the samples changes and with `false` once it has (after the lock
/// check of the tuner).
pub type RetuneEdge = std::sync::Arc<dyn Fn(bool) + Send + Sync>;

impl Rtl2832 {
    /// Claims interface 0 of `dev`, runs the USB and demodulator set-up, probes the tuner, sets the demodulator mode
    /// of the tuner and initialises the tuner. The gain mode is not set yet: [`Rtl2832::set_gain`] follows.
    ///
    /// # Errors
    ///
    /// [`RtlError::TunerNotSupported`] or [`RtlError::NoTuner`] for a dongle without a supported tuner, else the
    /// error of a transfer or of the tuner initialisation.
    pub fn open(dev: Box<dyn UsbDevice>) -> Result<Rtl2832, RtlError> {
        let mut c = Chip::new(dev);
        c.device().claim(0)?;
        power_up(&mut c)?;
        c.repeater(true)?;
        let found = probe(&mut c);
        let kind = match found {
            Ok(Some(k)) if k.supported() => k,
            other => {
                if !matches!(&other, Err(e) if e.unplugged()) {
                    let _ = c.repeater(false);
                    let _ = c.sys_write(DEMOD_CTL, 0x20);
                }
                return Err(match other {
                    Ok(Some(k)) => RtlError::TunerNotSupported(k.name()),
                    Ok(None) => RtlError::NoTuner,
                    Err(e) => e,
                });
            }
        };
        let info = c.info().clone();
        let board = match (&info.manufacturer, &info.product) {
            (None, _) | (_, None) => {
                let e = read_eeprom_image(&mut c)
                    .ok()
                    .and_then(|i| parse_eeprom(&i));
                board_of(
                    e.as_ref().map(|e| e.manufacturer.as_str()),
                    e.as_ref().map(|e| e.product.as_str()),
                )
            }
            (Some(m), Some(p)) => board_of(Some(m), Some(p)),
        };
        let tuner = match kind {
            TunerKind::E4000 => Tuner::E4000(E4000::new(XTAL_HZ)),
            TunerKind::R828D => {
                let xtal = if board == Board::BlogV4 {
                    XTAL_HZ
                } else {
                    16_000_000
                };
                Tuner::R82xx(R82xx::new(Chip82::R828D, board, xtal))
            }
            _ => Tuner::R82xx(R82xx::new(Chip82::R820T, board, XTAL_HZ)),
        };
        let mut d = Rtl2832 {
            chip: c,
            kind,
            board,
            tuner,
            xtal: XTAL_HZ,
            ppm: 0.0,
            rate: None,
            rate_eff: 0.0,
            freq: None,
            bandwidth: 0,
            if_hz: 0,
            direct: Direct::Off,
            offset: false,
            gain: Gain::Auto,
            rtl_agc: false,
            test_mode: false,
            bias: false,
            closed: false,
            edge: None,
        };
        let r = d.demod_mode().and_then(|()| d.tuner_init());
        let r = r.and_then(|()| d.chip.repeater(false));
        if let Err(e) = r {
            d.close();
            return Err(e);
        }
        Ok(d)
    }

    /// The demodulator input mode for the tuner (specification 3.6), with the repeater on.
    fn demod_mode(&mut self) -> Result<(), RtlError> {
        match &self.tuner {
            Tuner::R82xx(t) => {
                self.if_hz = t.if_hz();
                self.chip.demod_write1(1, 0xB1, 0x1A)?;
                self.chip.demod_write1(0, 0x08, 0x4D)?;
                self.write_if()?;
                self.chip.demod_write1(1, 0x15, 0x01)
            }
            Tuner::E4000(_) => {
                self.if_hz = 0;
                self.chip.demod_write1(1, 0xB1, 0x1B)?;
                self.chip.demod_write1(0, 0x08, 0xCD)?;
                self.chip.demod_write1(1, 0x15, 0x00)?;
                self.chip.demod_write1(0, 0x06, 0x80)?;
                self.write_if()
            }
        }
    }

    fn tuner_init(&mut self) -> Result<(), RtlError> {
        match &mut self.tuner {
            Tuner::R82xx(t) => t.init(&mut self.chip),
            Tuner::E4000(t) => t.init(&mut self.chip),
        }
    }

    /// Sets the function called around each change of the frequency of the samples.
    pub fn set_retune_edge(&mut self, edge: Option<RetuneEdge>) {
        self.edge = edge;
    }

    /// The tuner found.
    pub fn tuner(&self) -> TunerKind {
        self.kind
    }

    /// The board around the tuner.
    pub fn board(&self) -> Board {
        self.board
    }

    /// The device as the bus listed it.
    pub fn info(&self) -> &UsbDeviceInfo {
        self.chip.info()
    }

    /// The image of tuner registers 0x05 to 0x1F of an R820T-family or R828D tuner.
    pub fn r82xx_image(&self) -> Option<[u8; 27]> {
        match &self.tuner {
            Tuner::R82xx(t) => Some(t.image()),
            Tuner::E4000(_) => None,
        }
    }

    /// The PLL of the tuner locked at the last tune.
    pub fn locked(&self) -> bool {
        match &self.tuner {
            Tuner::R82xx(t) => t.locked(),
            Tuner::E4000(t) => t.locked(),
        }
    }

    /// The rate that the demodulator makes, in Hz; 0 before a rate is set.
    pub fn rate_hz(&self) -> f64 {
        self.rate_eff
    }

    /// The frequency tuned, in Hz.
    pub fn frequency(&self) -> Option<u64> {
        self.freq
    }

    /// The demodulator IF in Hz.
    pub fn if_hz(&self) -> u32 {
        self.if_hz
    }

    /// The direct sampling mode.
    pub fn direct(&self) -> Direct {
        self.direct
    }

    /// The crystal of the demodulator after the frequency correction.
    fn xtal_corrected(&self) -> u32 {
        corrected(self.xtal, self.ppm)
    }

    /// Writes the IF register for the IF of the demodulator.
    fn write_if(&mut self) -> Result<(), RtlError> {
        let (a, b, c) = if_register(self.if_hz, self.xtal_corrected());
        self.chip.demod_write1(1, 0x19, a)?;
        self.chip.demod_write1(1, 0x1A, b)?;
        self.chip.demod_write1(1, 0x1B, c)
    }

    /// The offset of offset tuning at the present rate.
    fn offset_hz(&self) -> u32 {
        self.rate.map_or(0, |r| r / 2 * 170 / 100)
    }

    /// The bandwidth the tuner filter is set to: the one set, twice the offset in offset tuning, else the rate.
    fn tuner_bandwidth(&self) -> u32 {
        if self.bandwidth > 0 {
            self.bandwidth
        } else if self.offset {
            2 * self.offset_hz()
        } else {
            self.rate.unwrap_or(0)
        }
    }

    /// Sets the tuner filter for [`Rtl2832::tuner_bandwidth`]; for an R820T-family tuner also the IF register and the
    /// tune that follow from the new IF. The repeater is on.
    fn apply_bandwidth(&mut self) -> Result<(), RtlError> {
        let bw = self.tuner_bandwidth();
        if bw == 0 || self.direct != Direct::Off {
            return Ok(());
        }
        match &mut self.tuner {
            Tuner::R82xx(t) => {
                let f = t.set_bandwidth(&mut self.chip, bw)?;
                self.if_hz = f.if_hz;
                self.write_if()?;
                self.tune_tuner()
            }
            Tuner::E4000(t) => t.set_bandwidth(&mut self.chip, bw),
        }
    }

    /// Tunes the tuner to the frequency set, if one is set. The repeater is on.
    fn tune_tuner(&mut self) -> Result<(), RtlError> {
        let Some(f) = self.freq else {
            return Ok(());
        };
        let off = if self.offset {
            u64::from(self.offset_hz())
        } else {
            0
        };
        let hook = self.edge.clone();
        let mut edge = |on: bool| {
            if let Some(h) = &hook {
                h(on);
            }
        };
        match &mut self.tuner {
            Tuner::R82xx(t) => t.set_frequency(&mut self.chip, f, &mut edge),
            Tuner::E4000(t) => t.set_frequency(&mut self.chip, f.saturating_sub(off), &mut edge),
        }
    }

    /// Sets the sample rate `fs` (specification 3.4): the tuner filter unless a bandwidth is set, the resampler
    /// ratio, the rate correction and a soft reset. Returns the effective rate.
    ///
    /// # Errors
    ///
    /// [`RtlError::Rate`] for a rate the chip does not make, else the error of a transfer or of the tune.
    pub fn set_sample_rate(&mut self, fs: u32) -> Result<f64, RtlError> {
        let (ratio, eff) = rate_ratio(fs, self.xtal)?;
        self.rate = Some(fs);
        self.rate_eff = eff;
        // The tuner filter follows the rate.
        if self.bandwidth == 0 && self.direct == Direct::Off {
            self.chip.repeater(true)?;
            let r = self.apply_bandwidth();
            self.repeater_off_after(r)?;
        }
        self.chip
            .demod_write(1, 0x9F, &[(ratio >> 24) as u8, (ratio >> 16) as u8])?;
        self.chip
            .demod_write(1, 0xA1, &[(ratio >> 8) as u8, ratio as u8])?;
        self.write_correction()?;
        self.soft_reset()?;
        if self.offset {
            self.apply_offset()?;
        }
        Ok(eff)
    }

    /// Switches the repeater off after the result `r` of a tuner call: always, except after an unplug.
    fn repeater_off_after<T>(&mut self, r: Result<T, RtlError>) -> Result<T, RtlError> {
        match r {
            Err(e) if e.unplugged() => Err(e),
            Err(e) => {
                let _ = self.chip.repeater(false);
                Err(e)
            }
            Ok(v) => {
                self.chip.repeater(false)?;
                Ok(v)
            }
        }
    }

    fn write_correction(&mut self) -> Result<(), RtlError> {
        let (r3e, r3f) = ppm_correction(self.ppm);
        self.chip.demod_write1(1, 0x3F, r3f)?;
        self.chip.demod_write1(1, 0x3E, r3e)
    }

    fn soft_reset(&mut self) -> Result<(), RtlError> {
        self.chip.demod_write1(1, 0x01, 0x14)?;
        self.chip.demod_write1(1, 0x01, 0x10)
    }

    /// Sets the bandwidth of the tuner filter in Hz; 0 follows the sample rate.
    ///
    /// # Errors
    ///
    /// The error of a transfer or of the tune.
    pub fn set_bandwidth(&mut self, bw: u32) -> Result<(), RtlError> {
        self.bandwidth = bw;
        if self.direct != Direct::Off {
            return Ok(());
        }
        self.chip.repeater(true)?;
        let r = self.apply_bandwidth();
        self.repeater_off_after(r)
    }

    /// Tunes to `hz` (specification 4.1.8, 4.2.3, 5.2 and 5.3).
    ///
    /// # Errors
    ///
    /// [`RtlError::NoPllSetting`] or [`RtlError::PllNotLocked`], else the error of a transfer.
    pub fn set_frequency(&mut self, hz: u64) -> Result<(), RtlError> {
        self.freq = Some(hz);
        if self.direct != Direct::Off {
            self.if_hz = hz.min(u64::from(self.xtal)) as u32;
            if let Some(h) = &self.edge {
                h(true);
            }
            let r = self.write_if();
            if let Some(h) = &self.edge {
                h(false);
            }
            return r;
        }
        self.chip.repeater(true)?;
        let r = self.tune_tuner();
        self.repeater_off_after(r)
    }

    /// Sets the tuner gain: automatic, or a manual gain in tenth dB. Returns the manual gain set.
    ///
    /// # Errors
    ///
    /// The error of a transfer.
    pub fn set_gain(&mut self, gain: Gain) -> Result<Option<i32>, RtlError> {
        self.gain = gain;
        if self.direct != Direct::Off {
            return Ok(None);
        }
        self.chip.repeater(true)?;
        let r = self.apply_gain();
        self.repeater_off_after(r)
    }

    fn apply_gain(&mut self) -> Result<Option<i32>, RtlError> {
        let c = &mut self.chip;
        match (&mut self.tuner, self.gain) {
            (Tuner::R82xx(t), Gain::Auto) => t.set_auto_gain(c).map(|()| None),
            (Tuner::R82xx(t), Gain::Manual(g)) => t.set_manual_gain(c, g).map(Some),
            (Tuner::E4000(t), Gain::Auto) => t.set_auto_gain(c).map(|()| None),
            (Tuner::E4000(t), Gain::Manual(g)) => t.set_manual_gain(c, g).map(Some),
        }
    }

    /// The gains the tuner offers, in tenth dB.
    pub fn gains(&self) -> &'static [i32] {
        match self.tuner {
            Tuner::R82xx(_) => &r82xx::GAINS,
            Tuner::E4000(_) => &e4000::GAINS,
        }
    }

    /// Sets the frequency correction to `ppm` parts per million (specification 3.5): the rate correction, the IF
    /// register, the reference of the tuner, and a tune.
    ///
    /// # Errors
    ///
    /// The error of a transfer or of the tune.
    pub fn set_ppm(&mut self, ppm: f64) -> Result<(), RtlError> {
        if ppm == self.ppm {
            return Ok(());
        }
        self.ppm = ppm;
        match &mut self.tuner {
            Tuner::R82xx(t) => t.set_ppm(ppm),
            Tuner::E4000(t) => t.set_ppm(ppm),
        }
        self.write_correction()?;
        self.write_if()?;
        if self.freq.is_some() {
            let f = self.freq.unwrap_or(0);
            self.set_frequency(f)?;
        }
        Ok(())
    }

    /// Switches direct sampling (specification 5.2) and tunes to `hz`. Leaving it initialises the tuner again and
    /// restores its gain mode, filter and IF.
    ///
    /// # Errors
    ///
    /// [`RtlError::NotHere`] on a Blog V4 or V4L (they upconvert HF), else the error of a transfer or of the tune.
    pub fn set_direct(&mut self, mode: Direct, hz: u64) -> Result<(), RtlError> {
        if mode == self.direct {
            return self.set_frequency(hz);
        }
        self.freq = Some(hz);
        if mode != Direct::Off && self.board != Board::Generic {
            return Err(RtlError::NotHere(
                "an RTL-SDR Blog V4 or V4L receives HF through its upconverter, not by direct sampling",
            ));
        }
        if mode != Direct::Off {
            if self.direct == Direct::Off {
                self.chip.repeater(true)?;
                let r = self.tuner_standby();
                self.repeater_off_after(r)?;
                self.chip.demod_write1(1, 0xB1, 0x1A)?;
                self.chip.demod_write1(1, 0x15, 0x00)?;
                self.chip.demod_write1(0, 0x08, 0x4D)?;
            }
            self.chip
                .demod_write1(0, 0x06, if mode == Direct::I { 0x80 } else { 0x90 })?;
            self.direct = mode;
        } else {
            self.direct = Direct::Off;
            self.chip.repeater(true)?;
            let r = self
                .tuner_init()
                .and_then(|()| self.apply_gain().map(|_| ()));
            self.repeater_off_after(r)?;
            match self.tuner {
                Tuner::R82xx(_) => {
                    self.if_hz = r82xx::INIT_IF_HZ;
                    self.write_if()?;
                    self.chip.demod_write1(1, 0x15, 0x01)?;
                }
                Tuner::E4000(_) => {
                    self.if_hz = 0;
                    self.write_if()?;
                    self.chip.demod_write1(0, 0x08, 0xCD)?;
                    self.chip.demod_write1(1, 0xB1, 0x1B)?;
                }
            }
            self.chip.demod_write1(0, 0x06, 0x80)?;
            self.chip.repeater(true)?;
            let r = self.apply_bandwidth();
            self.repeater_off_after(r)?;
            if self.offset {
                self.apply_offset()?;
            }
        }
        if let Some(f) = self.freq {
            self.set_frequency(f)?;
        }
        Ok(())
    }

    fn tuner_standby(&mut self) -> Result<(), RtlError> {
        match &mut self.tuner {
            Tuner::R82xx(t) => t.standby(&mut self.chip),
            Tuner::E4000(t) => t.standby(&mut self.chip),
        }
    }

    /// Switches offset tuning (specification 5.3), for a zero-IF tuner outside direct sampling.
    ///
    /// # Errors
    ///
    /// [`RtlError::NotHere`] for an R820T-family or R828D tuner or in direct sampling, else the error of a transfer.
    pub fn set_offset_tuning(&mut self, on: bool) -> Result<(), RtlError> {
        if on == self.offset {
            return Ok(());
        }
        if on && !self.kind.zero_if() {
            return Err(RtlError::NotHere(
                "offset tuning is for zero-IF tuners; this tuner has an IF of its own",
            ));
        }
        if on && self.direct != Direct::Off {
            return Err(RtlError::NotHere(
                "offset tuning does not work with direct sampling",
            ));
        }
        self.offset = on;
        self.apply_offset()
    }

    /// Writes the IF register, the tuner filter and the tune for the present offset state.
    fn apply_offset(&mut self) -> Result<(), RtlError> {
        self.if_hz = if self.offset { self.offset_hz() } else { 0 };
        self.write_if()?;
        self.chip.repeater(true)?;
        let r = self.apply_bandwidth().and_then(|()| self.tune_tuner());
        self.repeater_off_after(r)
    }

    /// Switches the bias tee on GPIO 0.
    ///
    /// # Errors
    ///
    /// The error of a transfer.
    pub fn set_bias_tee(&mut self, on: bool) -> Result<(), RtlError> {
        self.chip.gpio_output(0)?;
        self.chip.gpio_set(0, on)?;
        self.bias = on;
        Ok(())
    }

    /// Switches the digital AGC of the RTL2832U.
    ///
    /// # Errors
    ///
    /// The error of a transfer.
    pub fn set_rtl_agc(&mut self, on: bool) -> Result<(), RtlError> {
        self.rtl_agc = on;
        self.write_mode()
    }

    /// Switches the counter test mode: every byte of the stream is the previous byte plus 1.
    ///
    /// # Errors
    ///
    /// The error of a transfer.
    pub fn set_test_mode(&mut self, on: bool) -> Result<(), RtlError> {
        self.test_mode = on;
        self.write_mode()
    }

    fn write_mode(&mut self) -> Result<(), RtlError> {
        let v = if self.test_mode {
            0x03
        } else if self.rtl_agc {
            0x25
        } else {
            0x05
        };
        self.chip.demod_write1(0, 0x19, v)
    }

    /// Resets the FIFO of the endpoint and starts the data flow.
    ///
    /// # Errors
    ///
    /// The error of a transfer.
    pub fn reset_buffer(&mut self) -> Result<(), RtlError> {
        self.chip.usb_write(USB_EPA_CTL, &[0x10, 0x02])?;
        self.chip.usb_write(USB_EPA_CTL, &[0x00, 0x00])
    }

    /// The chip, for diagnostics: what is written through it is not known to the driver.
    pub fn chip_mut(&mut self) -> &mut Chip {
        &mut self.chip
    }

    /// The open device, for a bulk stream.
    pub fn device(&mut self) -> &mut dyn UsbDevice {
        self.chip.device()
    }

    /// Reads the EEPROM. It is never written.
    ///
    /// # Errors
    ///
    /// The error of a transfer; `Ok(None)` for an image without the signature.
    pub fn read_eeprom(&mut self) -> Result<Option<Eeprom>, RtlError> {
        Ok(parse_eeprom(&read_eeprom_image(&mut self.chip)?))
    }

    /// Closes the dongle: the bias tee off if this driver switched it on, the tuner in standby and the demodulator
    /// powered down. It stops at the first unplug. Safe to call twice.
    pub fn close(&mut self) {
        if self.closed {
            return;
        }
        self.closed = true;
        if self.bias && self.set_bias_tee(false).is_err_and(|e| e.unplugged()) {
            return;
        }
        if self.chip.repeater(true).is_err_and(|e| e.unplugged()) {
            return;
        }
        if self.tuner_standby().is_err_and(|e| e.unplugged()) {
            return;
        }
        if self.chip.repeater(false).is_err_and(|e| e.unplugged()) {
            return;
        }
        let _ = self.chip.sys_write(DEMOD_CTL, 0x20);
    }
}

impl Drop for Rtl2832 {
    fn drop(&mut self) {
        self.close();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fir_bytes_of_the_default_coefficients() {
        assert_eq!(
            fir_bytes(&FIR).unwrap(),
            [
                0xca, 0xdc, 0xd7, 0xd8, 0xe0, 0xf2, 0x0e, 0x35, 0x06, 0x50, 0x9c, 0x0d, 0x71, 0x11,
                0x14, 0x71, 0x74, 0x19, 0x41, 0xa5
            ]
        );
        let mut bad = FIR;
        bad[0] = 200;
        assert!(fir_bytes(&bad).is_err());
        bad = FIR;
        bad[15] = 3000;
        assert!(fir_bytes(&bad).is_err());
    }

    #[test]
    fn rate_formula_and_ranges() {
        let rows: [(u32, u32, f64); 11] = [
            (250_000, 0x0CCC_CCCC, 250_000.000_4),
            (300_000, 0x0800_0000, 300_000.0),
            (1_024_000, 0x0708_0000, 1_024_000.0),
            (1_400_000, 0x0524_9248, 1_400_000.019),
            (1_800_000, 0x0400_0000, 1_800_000.0),
            (1_920_000, 0x03C0_0000, 1_920_000.0),
            (2_048_000, 0x0384_0000, 2_048_000.0),
            (2_400_000, 0x0300_0000, 2_400_000.0),
            (2_560_000, 0x02D0_0000, 2_560_000.0),
            (2_880_000, 0x0280_0000, 2_880_000.0),
            (3_200_000, 0x0240_0000, 3_200_000.0),
        ];
        for (fs, ratio, eff) in rows {
            let (r, e) = rate_ratio(fs, XTAL_HZ).unwrap();
            assert_eq!(r, ratio, "rate {fs}");
            assert!((e - eff).abs() < 0.001, "rate {fs}: {e}");
        }
        for bad in [225_000, 300_001, 900_000, 3_200_001, 0] {
            assert_eq!(rate_ratio(bad, XTAL_HZ), Err(RtlError::Rate(bad)));
        }
        for good in [225_001, 900_001] {
            assert!(rate_ratio(good, XTAL_HZ).is_ok());
        }
    }

    #[test]
    fn ppm_correction_table() {
        for (ppm, r3e, r3f) in [
            (0.0, 0x00, 0x00),
            (1.0, 0x3F, 0xF0),
            (-1.0, 0x00, 0x10),
            (10.0, 0x3F, 0x59),
            (50.0, 0x3C, 0xBA),
            (-50.0, 0x03, 0x46),
        ] {
            assert_eq!(ppm_correction(ppm), (r3e, r3f), "ppm {ppm}");
        }
    }

    #[test]
    fn if_register_table() {
        assert_eq!(if_register(0, XTAL_HZ), (0x00, 0x00, 0x00));
        assert_eq!(if_register(1_815_000, XTAL_HZ), (0x3B, 0xF7, 0x78));
        assert_eq!(if_register(3_570_000, XTAL_HZ), (0x38, 0x11, 0x12));
        assert_eq!(if_register(4_570_000, XTAL_HZ), (0x35, 0xD8, 0x2E));
    }
}
