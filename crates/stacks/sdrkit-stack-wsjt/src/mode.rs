//! The modes of the family and the numbers of each: periods, symbol lengths, tone spacings, start offsets and search ranges.

use sdrkit_algo_sim::modulate::{self, Cpm};
use sdrkit_algo_sim::SimError;

/// How a transmission's amplitude starts and ends.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Ramp {
    /// Full amplitude from the first sample (plain FSK modes).
    None,
    /// A raised cosine over this fraction of the first and last symbol (FT8: 1/8).
    Edge(f64),
    /// A raised cosine over the whole first and last symbol (FT4, FT2: their ramp symbols).
    WholeSymbol,
    /// A linear ramp of this many seconds at both ends (JS8: 10 ms).
    Linear(f64),
}

/// The waveform of a mode: continuous-phase FSK, with Gaussian shaping (GFSK) when `bt` is not 0, and amplitude ramps.
#[derive(Clone, Debug)]
pub struct FskSpec {
    /// Seconds per symbol.
    pub symbol_s: f64,
    /// Hz between tones (negative: mirrored, tones run downwards).
    pub spacing_hz: f64,
    /// Gaussian shaping (0: none).
    pub bt: f64,
    /// The amplitude ramps.
    pub ramp: Ramp,
}

impl FskSpec {
    /// The modulator of this waveform at `rate` samples per second, from the shared CPM modulator: level `k` is tone `k`,
    /// `k` tone spacings above tone 0. Each burst runs from the start of its first symbol to the end of its last, with the
    /// first and last tones held beyond the ends.
    ///
    /// # Errors
    /// The errors of [`Cpm::gfsk`], [`Cpm::cpfsk`] and [`Cpm::with_ramp`] for values out of range.
    pub fn modulator(&self, rate: f64) -> Result<Cpm, SimError> {
        let sps = self.symbol_s * rate;
        // Level 1 moves the frequency by h Rs / 2: one tone spacing.
        let h = 2.0 * self.spacing_hz * self.symbol_s;
        let m = if self.bt > 0.0 {
            Cpm::gfsk(self.bt, h, sps)?
        } else {
            Cpm::cpfsk(h, sps)?
        };
        let ramp = match self.ramp {
            Ramp::None => modulate::Ramp::None,
            Ramp::Edge(frac) => modulate::Ramp::RaisedCosine(frac * sps),
            Ramp::WholeSymbol => modulate::Ramp::RaisedCosine(sps),
            Ramp::Linear(s) => modulate::Ramp::Linear(s * rate),
        };
        m.with_held_edges().with_ramp(ramp)
    }
}

/// A mode of the family.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    /// FT8.
    Ft8,
    /// FT4.
    Ft4,
    /// FT2.
    Ft2,
    /// WSPR.
    Wspr,
    /// JT65 (A, B, C).
    Jt65,
    /// JT9 (A).
    Jt9,
    /// JT4 (A to G).
    Jt4,
    /// Q65 (A to E; 15 to 300 s).
    Q65,
    /// FST4.
    Fst4,
    /// FST4W.
    Fst4w,
    /// MSK144.
    Msk144,
    /// JS8 (Normal, Fast, Turbo).
    Js8,
}

impl Mode {
    /// Every mode, in the order of the mode list.
    pub const ALL: [Mode; 12] = [
        Mode::Ft8,
        Mode::Ft4,
        Mode::Ft2,
        Mode::Wspr,
        Mode::Jt65,
        Mode::Jt9,
        Mode::Jt4,
        Mode::Q65,
        Mode::Fst4,
        Mode::Fst4w,
        Mode::Msk144,
        Mode::Js8,
    ];
    /// The mode id (`ft8` ...).
    pub fn id(self) -> &'static str {
        match self {
            Mode::Ft8 => "ft8",
            Mode::Ft4 => "ft4",
            Mode::Ft2 => "ft2",
            Mode::Wspr => "wspr",
            Mode::Jt65 => "jt65",
            Mode::Jt9 => "jt9",
            Mode::Jt4 => "jt4",
            Mode::Q65 => "q65",
            Mode::Fst4 => "fst4",
            Mode::Fst4w => "fst4w",
            Mode::Msk144 => "msk144",
            Mode::Js8 => "js8",
        }
    }
    /// The display name.
    pub fn name(self) -> &'static str {
        match self {
            Mode::Ft8 => "FT8",
            Mode::Ft4 => "FT4",
            Mode::Ft2 => "FT2",
            Mode::Wspr => "WSPR",
            Mode::Jt65 => "JT65",
            Mode::Jt9 => "JT9",
            Mode::Jt4 => "JT4",
            Mode::Q65 => "Q65",
            Mode::Fst4 => "FST4",
            Mode::Fst4w => "FST4W",
            Mode::Msk144 => "MSK144",
            Mode::Js8 => "JS8",
        }
    }
    /// The mode of an id.
    pub fn from_id(id: &str) -> Option<Mode> {
        Mode::ALL
            .into_iter()
            .find(|m| m.id() == id.to_ascii_lowercase())
    }
    /// The default period, seconds.
    pub fn default_period(self) -> f64 {
        match self {
            Mode::Ft8 | Mode::Msk144 | Mode::Js8 => 15.0,
            Mode::Ft4 => 7.5,
            Mode::Ft2 => 3.75,
            Mode::Wspr | Mode::Fst4w => 120.0,
            Mode::Jt65 | Mode::Jt9 | Mode::Jt4 | Mode::Q65 | Mode::Fst4 => 60.0,
        }
    }
    /// The periods a user can choose.
    pub fn periods(self) -> &'static [f64] {
        match self {
            Mode::Q65 => &[15.0, 30.0, 60.0, 120.0, 300.0],
            Mode::Fst4 | Mode::Fst4w => &[15.0, 30.0, 60.0, 120.0, 300.0, 900.0, 1800.0],
            Mode::Msk144 => &[15.0, 30.0],
            Mode::Js8 => &[15.0, 10.0, 6.0],
            m => match m {
                Mode::Ft8 => &[15.0],
                Mode::Ft4 => &[7.5],
                Mode::Ft2 => &[3.75],
                Mode::Wspr => &[120.0],
                _ => &[60.0],
            },
        }
    }
    /// The sub-mode letters (empty: none).
    pub fn submodes(self) -> &'static [&'static str] {
        match self {
            Mode::Jt65 => &["A", "B", "C"],
            Mode::Jt9 => &["A"],
            Mode::Jt4 => &["A", "B", "C", "D", "E", "F", "G"],
            Mode::Q65 => &["A", "B", "C", "D", "E"],
            Mode::Js8 => &["Normal", "Fast", "Turbo"],
            _ => &[],
        }
    }
    /// The default dial frequency (upper sideband), Hz.
    pub fn default_dial_hz(self) -> f64 {
        match self {
            Mode::Ft8 => 14.074e6,
            Mode::Ft4 => 14.080e6,
            Mode::Ft2 => 14.084e6,
            Mode::Wspr | Mode::Fst4w => 14.0956e6,
            Mode::Jt65 => 14.076e6,
            Mode::Jt9 | Mode::Js8 => 14.078e6,
            Mode::Jt4 => 144.120e6,
            Mode::Q65 => 50.275e6,
            Mode::Fst4 => 7.0386e6,
            Mode::Msk144 => 50.260e6,
        }
    }
    /// Whether the mode is searched upside down too.
    pub fn searches_mirrored(self) -> bool {
        matches!(self, Mode::Ft8 | Mode::Ft4 | Mode::Ft2 | Mode::Wspr)
    }
}

/// A mode as a setting or a column: its id is the key, its display name the label.
impl sdrkit_core::Choice for Mode {
    const CHOICES: &'static [sdrkit_core::ChoiceDecl] = &[
        sdrkit_core::ChoiceDecl::new("ft8", "FT8"),
        sdrkit_core::ChoiceDecl::new("ft4", "FT4"),
        sdrkit_core::ChoiceDecl::new("ft2", "FT2"),
        sdrkit_core::ChoiceDecl::new("wspr", "WSPR"),
        sdrkit_core::ChoiceDecl::new("jt65", "JT65"),
        sdrkit_core::ChoiceDecl::new("jt9", "JT9"),
        sdrkit_core::ChoiceDecl::new("jt4", "JT4"),
        sdrkit_core::ChoiceDecl::new("q65", "Q65"),
        sdrkit_core::ChoiceDecl::new("fst4", "FST4"),
        sdrkit_core::ChoiceDecl::new("fst4w", "FST4W"),
        sdrkit_core::ChoiceDecl::new("msk144", "MSK144"),
        sdrkit_core::ChoiceDecl::new("js8", "JS8"),
    ];
    fn index(self) -> u16 {
        self as u16
    }
    fn from_index(index: u16) -> Option<Self> {
        Mode::ALL.get(usize::from(index)).copied()
    }
}

/// A mode with its sub-mode and period: everything the slot clock, the encoder and the decoder need.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sub {
    /// The mode.
    pub mode: Mode,
    /// The sub-mode index (JT65 A=0..C=2, JT4 A..G, Q65 A..E, JS8 Normal=0, Fast=1, Turbo=2).
    pub sub: u8,
    /// The period, seconds.
    pub period: f64,
}

impl Sub {
    /// The mode at its default sub-mode and period.
    pub fn new(mode: Mode) -> Sub {
        Sub {
            mode,
            sub: 0,
            period: mode.default_period(),
        }
    }
    /// With a sub-mode (index). A sub-mode that the standard does not define at the present period is not taken.
    pub fn with_sub(mut self, sub: u8) -> Sub {
        if !(Sub { sub, ..self }).defined() {
            return self;
        }
        self.sub = sub;
        if self.mode == Mode::Js8 {
            self.period = [15.0, 10.0, 6.0][sub.min(2) as usize];
        }
        self
    }
    /// With a period (seconds), for the modes that have a choice. A period that the standard does not define for the present
    /// sub-mode is not taken.
    pub fn with_period(mut self, period: f64) -> Sub {
        if self.mode.periods().contains(&period)
            && self.mode != Mode::Js8
            && (Sub { period, ..self }).defined()
        {
            self.period = period;
        }
        self
    }
    /// Whether the standard defines this sub-mode at this period. Q65 has A to C at 15 s, A to D at 30 s and A to E at the
    /// longer periods (the widest signal is 1733 Hz).
    pub fn defined(&self) -> bool {
        match self.mode {
            Mode::Q65 => {
                let widest = match self.period as u32 {
                    15 => 2,
                    30 => 3,
                    _ => 4,
                };
                self.sub <= widest
            }
            _ => true,
        }
    }
    /// The sub-mode letter or name.
    pub fn sub_name(&self) -> &'static str {
        self.mode
            .submodes()
            .get(self.sub as usize)
            .copied()
            .unwrap_or("")
    }
    /// A short label: `FT8`, `JT65A`, `Q65-60A`, `FST4-60`, `JS8 Fast`.
    pub fn label(&self) -> String {
        match self.mode {
            Mode::Jt65 | Mode::Jt9 | Mode::Jt4 => {
                format!("{}{}", self.mode.name(), self.sub_name())
            }
            Mode::Q65 => format!("Q65-{}{}", self.period as u32, self.sub_name()),
            Mode::Fst4 | Mode::Fst4w | Mode::Msk144
                if self.period != self.mode.default_period() =>
            {
                format!("{}-{}", self.mode.name(), self.period as u32)
            }
            Mode::Js8 => format!("JS8 {}", self.sub_name()),
            _ => self.mode.name().to_string(),
        }
    }
    /// Samples per symbol at 12 kHz.
    pub fn samples_per_symbol_12k(&self) -> f64 {
        match self.mode {
            Mode::Ft8 => 1920.0,
            Mode::Ft4 => 576.0,
            Mode::Ft2 => 288.0,
            Mode::Wspr => 8192.0,
            Mode::Jt65 => 4096.0 * 12000.0 / 11025.0,
            Mode::Jt9 => 6912.0,
            Mode::Jt4 => 2520.0 * 12000.0 / 11025.0,
            Mode::Q65 => match self.period as u32 {
                15 => 1800.0,
                30 => 3600.0,
                60 => 7200.0,
                120 => 16000.0,
                _ => 41472.0,
            },
            Mode::Fst4 | Mode::Fst4w => match self.period as u32 {
                15 => 720.0,
                30 => 1680.0,
                60 => 3888.0,
                120 => 8200.0,
                300 => 21504.0,
                900 => 66560.0,
                _ => 134_400.0,
            },
            Mode::Msk144 => 6.0,
            Mode::Js8 => [1920.0, 1200.0, 600.0][self.sub.min(2) as usize],
        }
    }
    /// Seconds per symbol (MSK144: per channel bit).
    pub fn symbol_s(&self) -> f64 {
        self.samples_per_symbol_12k() / 12000.0
    }
    /// The tone spacing, Hz.
    pub fn spacing_hz(&self) -> f64 {
        match self.mode {
            Mode::Ft8 => 6.25,
            Mode::Ft4 => 12000.0 / 576.0,
            Mode::Ft2 => 12000.0 / 288.0,
            Mode::Wspr => 12000.0 / 8192.0,
            Mode::Jt65 => 11025.0 / 4096.0 * (1 << self.sub.min(2)) as f64,
            Mode::Jt9 => 12000.0 / 6912.0,
            Mode::Jt4 => 4.375 * (1 << self.sub.min(6)) as f64,
            Mode::Q65 => 12000.0 / self.samples_per_symbol_12k() * (1 << self.sub.min(4)) as f64,
            Mode::Fst4 | Mode::Fst4w => 12000.0 / self.samples_per_symbol_12k(),
            Mode::Msk144 => 1000.0,
            Mode::Js8 => 12000.0 / self.samples_per_symbol_12k(),
        }
    }
    /// Tones (MSK144: its two).
    pub fn num_tones(&self) -> usize {
        match self.mode {
            Mode::Ft8 | Mode::Js8 => 8,
            Mode::Ft4 | Mode::Ft2 | Mode::Wspr | Mode::Jt4 | Mode::Fst4 | Mode::Fst4w => 4,
            Mode::Jt65 => 66,
            Mode::Jt9 => 9,
            Mode::Q65 => 65,
            Mode::Msk144 => 2,
        }
    }
    /// Symbols of a transmission (FT4 / FT2 with their ramp symbols; MSK144: one 144-bit frame).
    pub fn num_symbols(&self) -> usize {
        match self.mode {
            Mode::Ft8 | Mode::Js8 => 79,
            Mode::Ft4 | Mode::Ft2 => 105,
            Mode::Wspr => 162,
            Mode::Jt65 => 126,
            Mode::Jt9 | Mode::Q65 => 85,
            Mode::Jt4 => 206,
            Mode::Fst4 | Mode::Fst4w => 160,
            Mode::Msk144 => 144,
        }
    }
    /// The length of a transmission, seconds (MSK144: one frame).
    pub fn tx_s(&self) -> f64 {
        self.num_symbols() as f64 * self.symbol_s()
    }
    /// The start of the transmission after the slot start, seconds (t0).
    pub fn t0(&self) -> f64 {
        match self.mode {
            Mode::Ft8 | Mode::Ft4 | Mode::Ft2 => 0.5,
            Mode::Wspr | Mode::Jt65 | Mode::Jt9 | Mode::Jt4 => 1.0,
            Mode::Q65 | Mode::Fst4 | Mode::Fst4w => {
                if self.period <= 15.0 {
                    0.5
                } else {
                    1.0
                }
            }
            Mode::Msk144 => 0.0,
            Mode::Js8 => [0.5, 0.2, 0.1][self.sub.min(2) as usize],
        }
    }
    /// The DT range searched, seconds.
    pub fn dt_range(&self) -> (f64, f64) {
        match self.mode {
            Mode::Ft8 | Mode::Ft4 | Mode::Wspr => (-2.5, 2.5),
            Mode::Ft2 => (-1.0, 1.0),
            Mode::Jt65 => (-3.0, 8.0),
            Mode::Jt9 => (-3.0, 4.0),
            Mode::Jt4 => (-3.0, 5.5),
            Mode::Q65 | Mode::Fst4 | Mode::Fst4w => {
                let d = (0.1 * self.period).min(3.0);
                (-d, d)
            }
            Mode::Msk144 => (0.0, self.period),
            Mode::Js8 => [(-1.5, 2.5), (-1.2, 2.0), (-0.8, 1.2)][self.sub.min(2) as usize],
        }
    }
    /// The audio window of one slot, seconds from the slot start: (start, end).
    pub fn window(&self) -> (f64, f64) {
        if self.mode == Mode::Msk144 {
            return (0.0, self.period);
        }
        let (lo, hi) = self.dt_range();
        let g = 2.0 * self.symbol_s();
        (self.t0() + lo - g, self.t0() + self.tx_s() + hi + g)
    }
    /// The occupied bandwidth, Hz (for band-edge checks).
    pub fn bandwidth_hz(&self) -> f64 {
        match self.mode {
            Mode::Ft8 => 50.0,
            Mode::Ft4 => 90.0,
            Mode::Ft2 => 170.0,
            Mode::Wspr => 6.0,
            Mode::Jt65 => 180.0 * (1 << self.sub.min(2)) as f64,
            Mode::Jt9 => 16.0,
            Mode::Jt4 => 18.0 * (1 << self.sub.min(6)) as f64,
            Mode::Q65 => 65.0 * self.spacing_hz(),
            Mode::Fst4 | Mode::Fst4w => 4.0 * self.spacing_hz(),
            Mode::Msk144 => 2400.0,
            Mode::Js8 => 8.0 * self.spacing_hz(),
        }
    }
    /// The width the signal spans from tone 0 to the top tone, Hz.
    pub fn span_hz(&self) -> f64 {
        (self.num_tones() as f64 - 1.0) * self.spacing_hz()
    }
    /// The FSK description of the transmitted tones.
    pub fn fsk(&self) -> FskSpec {
        let (bt, ramp) = match self.mode {
            Mode::Ft8 => (2.0, Ramp::Edge(0.125)),
            Mode::Ft4 | Mode::Ft2 => (1.0, Ramp::WholeSymbol),
            Mode::Fst4 | Mode::Fst4w => (2.0, Ramp::Edge(0.125)),
            Mode::Js8 => (0.0, Ramp::Linear(0.01)),
            _ => (0.0, Ramp::None),
        };
        FskSpec {
            symbol_s: self.symbol_s(),
            spacing_hz: if self.mode == Mode::Msk144 {
                1000.0
            } else {
                self.spacing_hz()
            },
            bt,
            ramp,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_numbers_of_the_spec() {
        let ft8 = Sub::new(Mode::Ft8);
        assert!((ft8.tx_s() - 12.64).abs() < 1e-9);
        assert!((Sub::new(Mode::Ft4).tx_s() - 5.04).abs() < 1e-9);
        assert!((Sub::new(Mode::Ft2).tx_s() - 2.52).abs() < 1e-9);
        assert!((Sub::new(Mode::Wspr).tx_s() - 110.592).abs() < 1e-3);
        assert!((Sub::new(Mode::Jt4).tx_s() - 47.088).abs() < 5e-3);
        assert!((Sub::new(Mode::Jt9).tx_s() - 48.96).abs() < 1e-3);
        assert!((Sub::new(Mode::Js8).with_sub(2).tx_s() - 3.95).abs() < 1e-9);
        assert_eq!(Sub::new(Mode::Js8).with_sub(1).period, 10.0);
        assert!((Sub::new(Mode::Jt4).with_sub(6).spacing_hz() - 280.0).abs() < 1e-9);
        assert_eq!(Sub::new(Mode::Q65).with_period(30.0).label(), "Q65-30A");
    }
}
