//! The sample formats on the network: the choice of the format a server sends, its native sample type, and the packing of
//! transmit samples.
//!
//! Standard: SoapySDR `Formats.h` (format names and layouts).

use sdrkit_core::Complex32;
use sdrkit_graph::stream::SampleType;

/// A format on the network that the driver takes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    /// float32 I, Q.
    Cf32,
    /// int16 I, Q.
    Cs16,
    /// int8 I, Q.
    Cs8,
    /// uint8 I, Q.
    Cu8,
}

impl Format {
    /// The SoapySDR format name.
    pub fn name(self) -> &'static str {
        match self {
            Format::Cf32 => "CF32",
            Format::Cs16 => "CS16",
            Format::Cs8 => "CS8",
            Format::Cu8 => "CU8",
        }
    }
    /// The format of a SoapySDR format name (any case). `None` for a format the driver does not take.
    pub fn from_name(s: &str) -> Option<Format> {
        Some(match s.trim().to_uppercase().as_str() {
            "CF32" => Format::Cf32,
            "CS16" => Format::Cs16,
            "CS8" => Format::Cs8,
            "CU8" => Format::Cu8,
            _ => return None,
        })
    }
    /// Bytes per complex sample.
    pub fn bytes(self) -> usize {
        match self {
            Format::Cf32 => 8,
            Format::Cs16 => 4,
            Format::Cs8 | Format::Cu8 => 2,
        }
    }
    /// The sample type of the ring.
    pub fn sample_type(self) -> SampleType {
        match self {
            Format::Cf32 => SampleType::Cf32,
            Format::Cs16 => SampleType::Cs16,
            Format::Cs8 => SampleType::Cs8,
            Format::Cu8 => SampleType::Cu8,
        }
    }
}

sdrkit_core::choice_enum! {
    /// The format requested from the server ([`crate::FORMAT`]).
    pub enum WireChoice {
        /// The radio's own format, if the driver takes it, else CS16.
        Auto = ("auto", "Auto (the radio's own)"),
        /// 16-bit.
        Cs16 = ("cs16", "CS16 (16-bit)"),
        /// 8-bit, half the data.
        Cs8 = ("cs8", "CS8 (8-bit, half the data)"),
        /// Float, twice the data.
        Cf32 = ("cf32", "CF32 (float, twice the data)"),
    }
}

/// Chooses the format the server sends from the choice, the radio's native format and the formats the server lists (`None`:
/// the list is unknown). It returns the format and, if the chosen format is not listed, a note for the log.
pub fn choose_format(
    choice: WireChoice,
    native: Option<&str>,
    listed: Option<&[String]>,
) -> (Format, Option<String>) {
    let has = |f: Format| listed.is_none_or(|l| l.iter().any(|s| s.eq_ignore_ascii_case(f.name())));
    let auto = || {
        if let Some(n) = native.and_then(Format::from_name) {
            if has(n) {
                return n;
            }
        }
        if has(Format::Cs16) {
            Format::Cs16
        } else {
            Format::Cf32
        }
    };
    let fixed = match choice {
        WireChoice::Auto => return (auto(), None),
        WireChoice::Cs16 => Format::Cs16,
        WireChoice::Cs8 => Format::Cs8,
        WireChoice::Cf32 => Format::Cf32,
    };
    if has(fixed) {
        return (fixed, None);
    }
    let a = auto();
    (
        a,
        Some(format!(
            "the radio behind this server does not stream {}; {} is used",
            fixed.name(),
            a.name()
        )),
    )
}

/// The left shift that brings CS16 samples of the native full scale `native_scale` to the full 16-bit scale. It is 0 unless
/// `format` is the native format, CS16, and the scale is a power of two below 32768 (a 12-bit radio has scale 2048 and shift 4).
pub fn cs16_shift(format: Format, native: Option<&str>, native_scale: f64) -> u32 {
    let is_native = native.and_then(Format::from_name) == Some(format);
    if !is_native || format != Format::Cs16 || !(1.0..32768.0).contains(&native_scale) {
        return 0;
    }
    let bits = native_scale.log2();
    if bits.fract() == 0.0 {
        15 - bits as u32
    } else {
        0
    }
}

/// Packs `iq` as CS16 (round(x x 32767), held to the 16-bit range), little endian, into `out`.
pub fn pack_cs16(iq: &[Complex32], out: &mut [u8]) {
    for (s, o) in iq.iter().zip(out.as_chunks_mut::<4>().0) {
        let i = (f64::from(s.re) * 32767.0).round().clamp(-32768.0, 32767.0) as i16;
        let q = (f64::from(s.im) * 32767.0).round().clamp(-32768.0, 32767.0) as i16;
        o[..2].copy_from_slice(&i.to_le_bytes());
        o[2..].copy_from_slice(&q.to_le_bytes());
    }
}

/// Packs `iq` as CF32, little endian, into `out`.
pub fn pack_cf32(iq: &[Complex32], out: &mut [u8]) {
    for (s, o) in iq.iter().zip(out.as_chunks_mut::<8>().0) {
        o[..4].copy_from_slice(&s.re.to_le_bytes());
        o[4..].copy_from_slice(&s.im.to_le_bytes());
    }
}
