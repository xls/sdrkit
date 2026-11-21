//! Frequency arithmetic of a converter ahead of the radio: an LNB, a down-converter or an up-converter with a fixed
//! local oscillator.

/// How the converter mixes the RF with its local oscillator (LO).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mixing {
    /// The LO is below the RF: IF = RF − LO.
    LoBelow,
    /// The LO is above the RF: IF = LO − RF, and the spectrum is mirrored.
    LoAbove,
    /// An up-converter: IF = RF + LO.
    Sum,
}

/// A converter with a local oscillator at `lo_hz` that mixes by `mixing`.
///
/// ```
/// use sdrkit_algo_dsp::converter::Converter;
/// let lnb = Converter::down(9750e6, 10489.5e6);
/// assert_eq!(lnb.if_of(10489.5e6), 739.5e6);
/// assert_eq!(lnb.rf_of(739.5e6), 10489.5e6);
/// ```
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Converter {
    /// The local oscillator, Hz.
    pub lo_hz: f64,
    /// How the RF and the LO mix.
    pub mixing: Mixing,
}

impl Converter {
    /// A down-converter for RF near `rf_hz`: [`Mixing::LoBelow`] if `lo_hz` is below `rf_hz`, else [`Mixing::LoAbove`].
    pub fn down(lo_hz: f64, rf_hz: f64) -> Self {
        let mixing = if lo_hz < rf_hz {
            Mixing::LoBelow
        } else {
            Mixing::LoAbove
        };
        Converter { lo_hz, mixing }
    }

    /// An up-converter: IF = RF + `lo_hz`.
    pub fn up(lo_hz: f64) -> Self {
        Converter {
            lo_hz,
            mixing: Mixing::Sum,
        }
    }

    /// The same converter with its LO at `lo_hz`, for example with a measured LO error added.
    pub fn with_lo(self, lo_hz: f64) -> Self {
        Converter { lo_hz, ..self }
    }

    /// Returns true if the converter mirrors the spectrum.
    pub fn inverted(&self) -> bool {
        self.mixing == Mixing::LoAbove
    }

    /// Returns the IF of the RF `rf_hz`.
    pub fn if_of(&self, rf_hz: f64) -> f64 {
        match self.mixing {
            Mixing::LoBelow => rf_hz - self.lo_hz,
            Mixing::LoAbove => self.lo_hz - rf_hz,
            Mixing::Sum => rf_hz + self.lo_hz,
        }
    }

    /// Returns the RF of the IF `if_hz`.
    pub fn rf_of(&self, if_hz: f64) -> f64 {
        match self.mixing {
            Mixing::LoBelow => self.lo_hz + if_hz,
            Mixing::LoAbove => self.lo_hz - if_hz,
            Mixing::Sum => if_hz - self.lo_hz,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_mixing_maps_rf_to_if_and_back() {
        let low = Converter::down(9750e6, 10489.5e6);
        let high = Converter::down(10600e6, 10489.5e6);
        let up = Converter::up(125e6);
        assert!(!low.inverted() && high.inverted() && !up.inverted());
        assert_eq!(low.if_of(10489.5e6), 739.5e6);
        assert_eq!(high.if_of(10489.5e6), 110.5e6);
        assert_eq!(up.if_of(7.1e6), 132.1e6);
        for c in [low, high, up] {
            assert_eq!(c.rf_of(c.if_of(10489.5e6)), 10489.5e6);
        }
        assert_eq!(
            low.with_lo(9750.001e6).if_of(10489.5e6),
            10489.5e6 - 9750.001e6
        );
    }
}
