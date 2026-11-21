//! The transmission parameters (FFT size, guard interval, constellation, hierarchy, code rates and channel bandwidth) and the
//! numbers that follow from them.
//!
//! Standard: ETSI EN 300 744 clause 4.4 and Annex G.

use sdrkit_core::{Choice, ChoiceDecl};

/// Implements [`Choice`] for a parameter enum from its `ALL` list (variant order) and the keys and labels in that order.
macro_rules! parameter_choice {
    ($t:ty, [$(($key:literal, $label:literal)),+ $(,)?]) => {
        impl Choice for $t {
            const CHOICES: &'static [ChoiceDecl] = &[$(ChoiceDecl::new($key, $label)),+];
            fn index(self) -> u16 {
                self as u16
            }
            fn from_index(index: u16) -> Option<Self> {
                Self::ALL.get(usize::from(index)).copied()
            }
        }
    };
}

parameter_choice!(Mode, [("2K", "2K"), ("8K", "8K")]);
parameter_choice!(
    Guard,
    [
        ("1/32", "1/32"),
        ("1/16", "1/16"),
        ("1/8", "1/8"),
        ("1/4", "1/4")
    ]
);
parameter_choice!(
    Constellation,
    [("QPSK", "QPSK"), ("16-QAM", "16-QAM"), ("64-QAM", "64-QAM")]
);
parameter_choice!(
    Hierarchy,
    [
        ("none", "None"),
        ("1", "α = 1"),
        ("2", "α = 2"),
        ("4", "α = 4")
    ]
);
parameter_choice!(
    CodeRate,
    [
        ("1/2", "1/2"),
        ("2/3", "2/3"),
        ("3/4", "3/4"),
        ("5/6", "5/6"),
        ("7/8", "7/8")
    ]
);
parameter_choice!(
    Bandwidth,
    [
        ("5", "5 MHz"),
        ("6", "6 MHz"),
        ("7", "7 MHz"),
        ("8", "8 MHz")
    ]
);

/// The transmission mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Mode {
    /// 2048-point FFT, 1705 carriers.
    K2,
    /// 8192-point FFT, 6817 carriers.
    K8,
    /// 4096-point FFT, 3409 carriers (DVB-H and DVB-SH; the discriminant is the TPS mode code 10).
    K4,
    /// 1024-point FFT, 853 carriers (DVB-SH; the discriminant is the TPS mode code 11).
    K1,
}

impl Mode {
    /// The DVB-T modes, smallest first.
    pub const ALL: [Mode; 2] = [Mode::K2, Mode::K8];
    /// The DVB-SH modes (ETSI EN 302 583 clause 5.7.4.1), smallest first.
    pub const SH: [Mode; 4] = [Mode::K1, Mode::K2, Mode::K4, Mode::K8];
    /// FFT size N.
    pub const fn fft(self) -> usize {
        match self {
            Mode::K1 => 1024,
            Mode::K2 => 2048,
            Mode::K4 => 4096,
            Mode::K8 => 8192,
        }
    }
    /// Carriers K.
    pub const fn carriers(self) -> usize {
        match self {
            Mode::K1 => 853,
            Mode::K2 => 1705,
            Mode::K4 => 3409,
            Mode::K8 => 6817,
        }
    }
    /// Data carriers per symbol.
    pub const fn data_carriers(self) -> usize {
        match self {
            Mode::K1 => 756,
            Mode::K2 => 1512,
            Mode::K4 => 3024,
            Mode::K8 => 6048,
        }
    }
    /// "1K", "2K", "4K" or "8K".
    pub fn text(self) -> &'static str {
        match self {
            Mode::K1 => "1K",
            Mode::K2 => "2K",
            Mode::K4 => "4K",
            Mode::K8 => "8K",
        }
    }
    /// From the FFT size.
    pub fn from_fft(n: usize) -> Option<Mode> {
        match n {
            1024 => Some(Mode::K1),
            2048 => Some(Mode::K2),
            4096 => Some(Mode::K4),
            8192 => Some(Mode::K8),
            _ => None,
        }
    }
}

/// The guard interval.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Guard {
    /// N/32.
    G32,
    /// N/16.
    G16,
    /// N/8.
    G8,
    /// N/4.
    G4,
}

impl Guard {
    /// All, shortest first (TPS codes 0 to 3).
    pub const ALL: [Guard; 4] = [Guard::G32, Guard::G16, Guard::G8, Guard::G4];
    /// The divisor (32, 16, 8, 4).
    pub const fn divisor(self) -> usize {
        match self {
            Guard::G32 => 32,
            Guard::G16 => 16,
            Guard::G8 => 8,
            Guard::G4 => 4,
        }
    }
    /// Guard samples for FFT size `n`.
    pub const fn samples(self, n: usize) -> usize {
        n / self.divisor()
    }
    /// "1/4" ...
    pub fn text(self) -> &'static str {
        match self {
            Guard::G32 => "1/32",
            Guard::G16 => "1/16",
            Guard::G8 => "1/8",
            Guard::G4 => "1/4",
        }
    }
    /// The TPS code.
    pub fn code(self) -> u8 {
        self as u8
    }
    /// From the TPS code.
    pub fn from_code(c: u8) -> Guard {
        Guard::ALL[c as usize & 3]
    }
}

/// The constellation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Constellation {
    /// QPSK.
    Qpsk,
    /// 16-QAM.
    Qam16,
    /// 64-QAM.
    Qam64,
}

impl Constellation {
    /// All.
    pub const ALL: [Constellation; 3] = [
        Constellation::Qpsk,
        Constellation::Qam16,
        Constellation::Qam64,
    ];
    /// Bits per cell v.
    pub const fn bits(self) -> usize {
        match self {
            Constellation::Qpsk => 2,
            Constellation::Qam16 => 4,
            Constellation::Qam64 => 6,
        }
    }
    /// "QPSK", "16-QAM", "64-QAM".
    pub fn text(self) -> &'static str {
        match self {
            Constellation::Qpsk => "QPSK",
            Constellation::Qam16 => "16-QAM",
            Constellation::Qam64 => "64-QAM",
        }
    }
}

/// The hierarchy (α).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Hierarchy {
    /// Non-hierarchical.
    None,
    /// α = 1.
    A1,
    /// α = 2.
    A2,
    /// α = 4.
    A4,
}

impl Hierarchy {
    /// All.
    pub const ALL: [Hierarchy; 4] = [Hierarchy::None, Hierarchy::A1, Hierarchy::A2, Hierarchy::A4];
    /// α (1 for non-hierarchical).
    pub const fn alpha(self) -> usize {
        match self {
            Hierarchy::None | Hierarchy::A1 => 1,
            Hierarchy::A2 => 2,
            Hierarchy::A4 => 4,
        }
    }
    /// Whether the multiplex is hierarchical.
    pub fn hierarchical(self) -> bool {
        self != Hierarchy::None
    }
    /// "none", "1", "2", "4".
    pub fn text(self) -> &'static str {
        match self {
            Hierarchy::None => "none",
            Hierarchy::A1 => "1",
            Hierarchy::A2 => "2",
            Hierarchy::A4 => "4",
        }
    }
}

/// The inner code rate.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum CodeRate {
    /// 1/2.
    R12,
    /// 2/3.
    R23,
    /// 3/4.
    R34,
    /// 5/6.
    R56,
    /// 7/8.
    R78,
}

impl CodeRate {
    /// All (TPS codes 0 to 4).
    pub const ALL: [CodeRate; 5] = [
        CodeRate::R12,
        CodeRate::R23,
        CodeRate::R34,
        CodeRate::R56,
        CodeRate::R78,
    ];
    /// (input bits, output bits) per puncturing period.
    pub const fn ratio(self) -> (usize, usize) {
        match self {
            CodeRate::R12 => (1, 2),
            CodeRate::R23 => (2, 3),
            CodeRate::R34 => (3, 4),
            CodeRate::R56 => (5, 6),
            CodeRate::R78 => (7, 8),
        }
    }
    /// The rate as a number.
    pub fn value(self) -> f64 {
        let (k, n) = self.ratio();
        k as f64 / n as f64
    }
    /// The transmitted order of one period: (trellis step, Y (true) or X).
    pub fn pattern(self) -> &'static [(u8, bool)] {
        match self {
            CodeRate::R12 => &[(0, false), (0, true)],
            CodeRate::R23 => &[(0, false), (0, true), (1, true)],
            CodeRate::R34 => &[(0, false), (0, true), (1, true), (2, false)],
            CodeRate::R56 => &[
                (0, false),
                (0, true),
                (1, true),
                (2, false),
                (3, true),
                (4, false),
            ],
            CodeRate::R78 => &[
                (0, false),
                (0, true),
                (1, true),
                (2, true),
                (3, true),
                (4, false),
                (5, true),
                (6, false),
            ],
        }
    }
    /// "2/3" ...
    pub fn text(self) -> &'static str {
        match self {
            CodeRate::R12 => "1/2",
            CodeRate::R23 => "2/3",
            CodeRate::R34 => "3/4",
            CodeRate::R56 => "5/6",
            CodeRate::R78 => "7/8",
        }
    }
    /// The TPS code.
    pub fn code(self) -> u8 {
        self as u8
    }
    /// From the TPS code (`None` for reserved values).
    pub fn from_code(c: u8) -> Option<CodeRate> {
        CodeRate::ALL.get(c as usize).copied()
    }
}

/// The channel bandwidth.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Bandwidth {
    /// 5 MHz (Annex G).
    B5,
    /// 6 MHz.
    B6,
    /// 7 MHz.
    B7,
    /// 8 MHz.
    B8,
}

impl Bandwidth {
    /// All.
    pub const ALL: [Bandwidth; 4] = [Bandwidth::B5, Bandwidth::B6, Bandwidth::B7, Bandwidth::B8];
    /// MHz.
    pub const fn mhz(self) -> u32 {
        match self {
            Bandwidth::B5 => 5,
            Bandwidth::B6 => 6,
            Bandwidth::B7 => 7,
            Bandwidth::B8 => 8,
        }
    }
    /// The natural sample rate 1/T: 8/7 × the bandwidth.
    pub fn natural_rate(self) -> f64 {
        self.mhz() as f64 * 8e6 / 7.0
    }
    /// The lowest input rate the receiver accepts: 7.9 MHz × bandwidth / 8 MHz.
    pub fn min_rate(self) -> f64 {
        7.9e6 * self.mhz() as f64 / 8.0
    }
    /// From MHz.
    pub fn from_mhz(m: u32) -> Option<Bandwidth> {
        Bandwidth::ALL.into_iter().find(|b| b.mhz() == m)
    }
}

/// Every parameter TPS signals.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TpsParams {
    /// Transmission mode.
    pub mode: Mode,
    /// Guard interval.
    pub guard: Guard,
    /// Constellation.
    pub constellation: Constellation,
    /// Hierarchy.
    pub hierarchy: Hierarchy,
    /// HP code rate.
    pub rate_hp: CodeRate,
    /// LP code rate.
    pub rate_lp: CodeRate,
    /// Cell identifier (16 bits).
    pub cell_id: u16,
    /// Length indicator (23 or 31).
    pub length: u8,
    /// DVB-H signalling bits s48 to s53.
    pub dvbh: u8,
}

impl Default for TpsParams {
    fn default() -> Self {
        TpsParams {
            mode: Mode::K2,
            guard: Guard::G8,
            constellation: Constellation::Qam16,
            hierarchy: Hierarchy::None,
            rate_hp: CodeRate::R23,
            rate_lp: CodeRate::R23,
            cell_id: 0,
            length: 23,
            dvbh: 0,
        }
    }
}

impl TpsParams {
    /// Guard samples.
    pub fn guard_samples(&self) -> usize {
        self.guard.samples(self.mode.fft())
    }
    /// Samples per OFDM symbol (N + G).
    pub fn symbol_samples(&self) -> usize {
        self.mode.fft() + self.guard_samples()
    }
    /// The code rate of a stream (`lp`: the low-priority one).
    pub fn rate(&self, lp: bool) -> CodeRate {
        if lp && self.hierarchy.hierarchical() {
            self.rate_lp
        } else {
            self.rate_hp
        }
    }
    /// Bits per cell of a stream.
    pub fn stream_bits(&self, lp: bool) -> usize {
        if self.hierarchy.hierarchical() {
            if lp {
                self.constellation.bits() - 2
            } else {
                2
            }
        } else {
            self.constellation.bits()
        }
    }
    /// The net transport stream bit rate of a stream at bandwidth `bw` (EN 300 744 Table 17).
    pub fn net_bitrate(&self, bw: Bandwidth, lp: bool) -> f64 {
        let bits = (self.mode.data_carriers() * self.stream_bits(lp)) as f64
            * self.rate(lp).value()
            * 188.0
            / 204.0;
        bits * bw.natural_rate() / self.symbol_samples() as f64
    }
    /// The mode text: "8K 1/8 64-QAM 2/3".
    pub fn text(&self) -> String {
        let mut s = format!(
            "{} {} {} {}",
            self.mode.text(),
            self.guard.text(),
            self.constellation.text(),
            self.rate_hp.text()
        );
        if self.hierarchy.hierarchical() {
            s.push_str(&format!(
                " α{} LP {}",
                self.hierarchy.text(),
                self.rate_lp.text()
            ));
        }
        s
    }
    /// The duration of a frame (68 symbols) in seconds.
    pub fn frame_s(&self, bw: Bandwidth) -> f64 {
        68.0 * self.symbol_samples() as f64 / bw.natural_rate()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn net_bitrates_of_the_spec() {
        let p = TpsParams {
            mode: Mode::K8,
            guard: Guard::G4,
            constellation: Constellation::Qam64,
            rate_hp: CodeRate::R23,
            ..Default::default()
        };
        assert!((p.net_bitrate(Bandwidth::B8, false) / 1e6 - 19.91).abs() < 0.01);
        let p = TpsParams {
            mode: Mode::K2,
            guard: Guard::G8,
            constellation: Constellation::Qam16,
            rate_hp: CodeRate::R23,
            ..Default::default()
        };
        assert!((p.net_bitrate(Bandwidth::B7, false) - 12_901_960.8).abs() < 1.0);
        assert!((Bandwidth::B8.natural_rate() - 64e6 / 7.0).abs() < 1e-6);
        assert!((Bandwidth::B6.min_rate() - 5.925e6).abs() < 1.0);
    }
}
