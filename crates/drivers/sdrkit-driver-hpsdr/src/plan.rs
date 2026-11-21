//! The rules of the HPSDR drivers that need no network: the rate chosen for a requested rate, the frequency sent, the gain, the
//! N2ADR and Alex filters, the transmit levels and the notes.
//! Standard: HPSDR USB data protocol (protocol 1), openHPSDR Ethernet protocol (protocol 2).

use crate::profile::{GainKind, Profile};
use sdrkit_core::{Hz, SettingSet};
use sdrkit_driver_core::{DriverError, RadioSetting};

/// The highest frequency word protocol 1 can say, a little below 2^32 Hz.
pub const MAX_FREQ_HZ: f64 = 4_290_000_000.0;

/// The rate chosen for `asked`: the lowest offered rate at or above `asked` x 0.9999, or the highest when `asked` is 0. With
/// `exact`, only an offered rate is taken.
///
/// # Errors
///
/// [`DriverError::Refused`] for [`RadioSetting::Rate`] if `asked` is above every offered rate, or if `exact` is set and
/// `asked` is not offered.
pub fn choose_rate(asked: Hz, offered: &[u32], exact: bool) -> Result<u32, DriverError> {
    let refused = DriverError::Refused {
        id: RadioSetting::Rate.id(),
    };
    let top = offered.last().copied().ok_or(refused.clone())?;
    if asked <= 0.0 {
        return Ok(top);
    }
    let rate = offered
        .iter()
        .copied()
        .find(|&r| f64::from(r) >= asked * 0.9999)
        .ok_or(refused.clone())?;
    if exact && (f64::from(rate) - asked).abs() > asked * 1e-4 {
        return Err(refused);
    }
    Ok(rate)
}

/// The protocol 1 rate code of register 0x00 `C1[1:0]` (0 for 48 ksps to 3 for 384 ksps).
pub fn rate_code(rate: u32) -> u8 {
    match rate {
        r if r >= 384_000 => 3,
        r if r >= 192_000 => 2,
        r if r >= 96_000 => 1,
        _ => 0,
    }
}

/// The frequency sent for a centre: corrected for the clock and rounded to whole Hz. `None` if it is above [`MAX_FREQ_HZ`].
pub fn freq_sent(centre: Hz, ppm: f64) -> Option<u32> {
    let f = (centre / (1.0 + ppm * 1e-6)).round();
    (f.is_finite() && (0.0..=MAX_FREQ_HZ).contains(&f)).then_some(f as u32)
}

/// The note for a frequency above the profile's top, or empty.
pub fn freq_note(p: &Profile, sent: u32) -> String {
    if sent as f64 > p.max_freq_hz + 1.0 {
        format!(
            "the {} is specified for up to {:.1} MHz; the driver may not reach {:.3} MHz or the signal may be weak or absent",
            p.name,
            p.max_freq_hz / 1e6,
            sent as f64 / 1e6
        )
    } else {
        String::new()
    }
}

/// What a receiver's gain setting requires from the radio.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Gain {
    /// Hermes-Lite 2 LNA gain, -12 to +48 dB.
    Lna(i32),
    /// Step attenuation, 0 to 31 dB.
    Atten(u8),
    /// A factor on the sample words, in units of 1/65536 ([`UNITY`] for 0 dB).
    Words(u32),
    /// No gain control.
    None,
}

/// The factor of [`Gain::Words`] for 0 dB.
pub const UNITY: u32 = 1 << 16;

/// The factor of [`Gain::Words`] for `db` dB.
pub fn words_factor(db: f64) -> u32 {
    (10f64.powf(db / 20.0) * f64::from(UNITY)).round() as u32
}

/// `word` times `factor` / 65536, held to the range of an `i32`.
#[inline]
pub fn scale_word(word: i32, factor: u32) -> i32 {
    ((i64::from(word) * i64::from(factor)) >> 16).clamp(i64::from(i32::MIN), i64::from(i32::MAX))
        as i32
}

/// The LNA gain while the gain is automatic: the radio has no automatic gain control of its own.
pub const LNA_DEFAULT_DB: i32 = 20;

/// The gain for the profile `p` with a gain control (`has_gain`) and a hand gain in dB (`None`: automatic). A radio without a
/// gain control returns [`Gain::None`] for any gain.
///
/// # Errors
///
/// [`DriverError::Refused`] for [`RadioSetting::Gain`] if the hand gain is outside the range of the radio.
pub fn gain(p: &Profile, has_gain: bool, gain_db: Option<f64>) -> Result<Gain, DriverError> {
    if !has_gain {
        return Ok(Gain::None);
    }
    let refused = || DriverError::Refused {
        id: RadioSetting::Gain.id(),
    };
    let fits = |g: f64, lo: f64, hi: f64| (lo..=hi).contains(&g).then_some(g.round());
    Ok(match (p.gain, gain_db) {
        (GainKind::Lna, None) => Gain::Lna(LNA_DEFAULT_DB),
        (GainKind::Lna, Some(g)) => Gain::Lna(fits(g, -12.0, 48.0).ok_or_else(refused)? as i32),
        (GainKind::Attenuator, None) => Gain::Atten(0),
        (GainKind::Attenuator, Some(g)) => {
            Gain::Atten(-fits(g, -31.0, 0.0).ok_or_else(refused)? as u8)
        }
        (GainKind::Words, None) => Gain::Words(UNITY),
        (GainKind::Words, Some(g)) => {
            Gain::Words(words_factor(fits(g, 0.0, 40.0).ok_or_else(refused)?))
        }
    })
}

/// Register 0x0a C4 for a Hermes-Lite 2 LNA gain: 0x40 | (gain + 12).
pub fn lna_byte(db: i32) -> u8 {
    0x40 | (db.clamp(-12, 48) + 12) as u8
}

/// The N2ADR low-pass filter bits of register 0x00 C2 for the highest frequency in use.
pub fn n2adr_bits(f: u32) -> u8 {
    match f {
        f if f < 2_000_000 => 0x02,
        f if f < 4_000_000 => 0x04,
        f if f < 8_000_000 => 0x08,
        f if f < 16_000_000 => 0x10,
        f if f < 24_000_000 => 0x20,
        f if f < 30_000_000 => 0x40,
        _ => 0x00,
    }
}

/// Alex0 word bits of the protocol 2 high-priority packet (from the openHPSDR Ethernet protocol document).
pub mod alex {
    /// 13 MHz high-pass.
    pub const HPF_13: u32 = 1 << 1;
    /// 20 MHz high-pass.
    pub const HPF_20: u32 = 1 << 2;
    /// 6 m preamplifier.
    pub const PREAMP_6M: u32 = 1 << 3;
    /// 9.5 MHz high-pass.
    pub const HPF_9_5: u32 = 1 << 4;
    /// 6.5 MHz high-pass.
    pub const HPF_6_5: u32 = 1 << 5;
    /// 1.5 MHz high-pass.
    pub const HPF_1_5: u32 = 1 << 6;
    /// High-pass filters bypassed.
    pub const HPF_BYPASS: u32 = 1 << 12;
    /// 30/20 m low-pass.
    pub const LPF_30_20: u32 = 1 << 20;
    /// 60/40 m low-pass.
    pub const LPF_60_40: u32 = 1 << 21;
    /// 80 m low-pass.
    pub const LPF_80: u32 = 1 << 22;
    /// 160 m low-pass.
    pub const LPF_160: u32 = 1 << 23;
    /// 6 m low-pass (bypass).
    pub const LPF_6: u32 = 1 << 29;
    /// 12/10 m low-pass.
    pub const LPF_12_10: u32 = 1 << 30;
    /// 17/15 m low-pass.
    pub const LPF_17_15: u32 = 1 << 31;
}

/// The Alex0 high-pass bits for the receive frequency of DDC0.
pub fn alex_hpf(f: u32) -> u32 {
    match f {
        f if f < 1_500_000 => alex::HPF_BYPASS,
        f if f < 6_500_000 => alex::HPF_1_5,
        f if f < 9_500_000 => alex::HPF_6_5,
        f if f < 13_000_000 => alex::HPF_9_5,
        f if f < 20_000_000 => alex::HPF_13,
        f if f <= 50_000_000 => alex::HPF_20,
        _ => alex::PREAMP_6M,
    }
}

/// The Alex0 low-pass bits for a transmit frequency.
pub fn alex_lpf(f: u32) -> u32 {
    match f {
        f if f < 2_500_000 => alex::LPF_160,
        f if f < 5_000_000 => alex::LPF_80,
        f if f < 8_000_000 => alex::LPF_60_40,
        f if f < 16_500_000 => alex::LPF_30_20,
        f if f < 24_000_000 => alex::LPF_17_15,
        f if f < 35_000_000 => alex::LPF_12_10,
        _ => alex::LPF_6,
    }
}

/// A protocol 2 phase word for a frequency: round(f x 2^32 / 122 880 000), clamped to 32 bits.
pub fn phase_word(f: u32) -> u32 {
    let w = (f as f64 * 4_294_967_296.0 / 122_880_000.0).round();
    if w >= 4_294_967_295.0 {
        u32::MAX
    } else {
        w as u32
    }
}

/// The SWR for forward and reverse readings, when forward > reverse > 0.
pub fn swr(fwd: u16, rev: u16) -> Option<f64> {
    if fwd > rev && rev > 0 {
        let r = (rev as f64 / fwd as f64).sqrt();
        Some((1.0 + r) / (1.0 - r))
    } else {
        None
    }
}

/// The Hermes-Lite 2 temperature in degrees C from its 12-bit reading.
pub fn hl2_temperature(raw: u16) -> f64 {
    (3.26 * raw as f64 / 4096.0 - 0.5) / 0.01
}

/// Transmit drive: 0 to 100 % mapped to 0 to 255, rounded.
pub fn drive_byte(pct: f64) -> u8 {
    if !pct.is_finite() {
        return 0;
    }
    (pct.clamp(0.0, 100.0) / 100.0 * 255.0).round() as u8
}

/// A 16-bit transmit sample: round(clamp(x, -1, 1) x 32767).
pub fn tx16(x: f32) -> i16 {
    if !x.is_finite() {
        return 0;
    }
    (x.clamp(-1.0, 1.0) * 32767.0).round() as i16
}

/// A 24-bit transmit sample: round(clamp(x, -1, 1) x 8 388 607).
pub fn tx24(x: f32) -> i32 {
    if !x.is_finite() {
        return 0;
    }
    (x.clamp(-1.0, 1.0) as f64 * 8_388_607.0).round() as i32
}

/// Joins notes that are not empty with "; ".
pub fn join_notes(notes: &[String]) -> String {
    notes
        .iter()
        .filter(|n| !n.is_empty())
        .cloned()
        .collect::<Vec<_>>()
        .join("; ")
}
