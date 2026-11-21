//! The demodulators and the typed items that flow between the components of the general receiver.

use sdrkit_algo_modem::analog::LowPass;
use sdrkit_core::Command;
use sdrkit_graph::Item;

/// The command that starts the work of a component again from empty history (a channel move, a new demodulator).
pub const RESTART: Command =
    Command::new("restart", "Restart").help("Starts again from empty history.");

sdrkit_core::commands! {
    /// The commands of a component that only restarts.
    pub enum RestartCommand {
        /// Starts again from empty history.
        Restart = RESTART,
    }
}

sdrkit_core::choice_enum! {
    /// The demodulator of the general receiver.
    #[derive(Default)]
    pub enum Demod {
        /// Narrow FM (land mobile, amateur), 5 kHz full deviation.
        #[default]
        Nfm = ("nfm", "NFM"),
        /// Wide FM, mono (broadcast), 75 kHz full deviation.
        Wfm = ("wfm", "WFM"),
        /// AM: envelope, or the AM processor (synchronous detection and Auto quality).
        Am = ("am", "AM"),
        /// Upper sideband.
        Usb = ("usb", "USB"),
        /// Lower sideband.
        Lsb = ("lsb", "LSB"),
        /// CW with a beat-frequency oscillator.
        Cw = ("cw", "CW"),
    }
}

sdrkit_core::choice_enum! {
    /// The audio low-pass of NFM. FM noise power grows as the cube of the audio bandwidth, so 3 kHz has about 3.7 dB less
    /// noise than 4 kHz and keeps speech up to about 2.7 kHz at its level.
    #[derive(Default)]
    pub enum NfmLowPass {
        /// -6 dB at 3 kHz.
        #[default]
        Khz3 = ("3khz", "3 kHz"),
        /// -6 dB at 4 kHz.
        Khz4 = ("4khz", "4 kHz"),
    }
}

impl NfmLowPass {
    /// The -6 dB point (Hz).
    pub fn cutoff_hz(self) -> f64 {
        match self {
            NfmLowPass::Khz3 => 3000.0,
            NfmLowPass::Khz4 => 4000.0,
        }
    }
    /// The filter: flat to 0.9 of the cut-off, 60 dB down from 1.1 of it. Both choices have the same shape, so their noise
    /// differs by the cube of their cut-offs.
    pub fn filter(self) -> LowPass {
        let c = self.cutoff_hz();
        LowPass::Edges {
            pass_hz: 0.9 * c,
            stop_hz: 1.1 * c,
            atten_db: 60.0,
        }
    }
}

/// The rate of every narrow demodulator.
pub const DEMOD_RATE: f64 = 48_000.0;
/// The rate of the WFM discriminator.
pub const WFM_RATE: f64 = 240_000.0;
/// The audio rate.
pub const AUDIO_RATE: f64 = 48_000.0;

impl Demod {
    /// Every demodulator, in the order of the setting's choices.
    pub const ALL: [Demod; 6] = [
        Demod::Nfm,
        Demod::Wfm,
        Demod::Am,
        Demod::Usb,
        Demod::Lsb,
        Demod::Cw,
    ];

    /// The stored key (`"nfm"` ...).
    pub fn id(self) -> &'static str {
        match self {
            Demod::Nfm => "nfm",
            Demod::Wfm => "wfm",
            Demod::Am => "am",
            Demod::Usb => "usb",
            Demod::Lsb => "lsb",
            Demod::Cw => "cw",
        }
    }
    /// The label (`"NFM"` ...).
    pub fn label(self) -> &'static str {
        match self {
            Demod::Nfm => "NFM",
            Demod::Wfm => "WFM",
            Demod::Am => "AM",
            Demod::Usb => "USB",
            Demod::Lsb => "LSB",
            Demod::Cw => "CW",
        }
    }
    /// The place in [`Demod::ALL`], for tables kept per demodulator.
    pub fn slot(self) -> usize {
        self as usize
    }
    /// WFM (the wide path at 240 kHz).
    pub fn is_wide(self) -> bool {
        self == Demod::Wfm
    }
    /// NFM or WFM.
    pub fn is_fm(self) -> bool {
        matches!(self, Demod::Nfm | Demod::Wfm)
    }
    /// USB or LSB.
    pub fn is_ssb(self) -> bool {
        matches!(self, Demod::Usb | Demod::Lsb)
    }
    /// The demodulation rate: 48 kHz, or 240 kHz for WFM.
    pub fn rate(self) -> f64 {
        if self.is_wide() {
            WFM_RATE
        } else {
            DEMOD_RATE
        }
    }
    /// The width presets (Hz).
    pub fn presets(self) -> &'static [f64] {
        match self {
            Demod::Nfm => &[6250.0, 12_500.0, 25_000.0],
            Demod::Wfm => &[150_000.0, 180_000.0, 200_000.0],
            Demod::Am => &[4000.0, 6000.0, 9000.0, 12_000.0],
            Demod::Usb | Demod::Lsb => &[1800.0, 2400.0, 2700.0, 3000.0],
            Demod::Cw => &[100.0, 250.0, 500.0, 1000.0],
        }
    }
    /// The default width (Hz).
    pub fn default_width(self) -> f64 {
        match self {
            Demod::Nfm => 12_500.0,
            Demod::Wfm => 180_000.0,
            Demod::Am => 9000.0,
            Demod::Usb | Demod::Lsb => 2700.0,
            Demod::Cw => 500.0,
        }
    }
    /// The range of a custom width (Hz).
    pub fn width_range(self) -> (f64, f64) {
        match self {
            Demod::Nfm => (2000.0, 30_000.0),
            Demod::Wfm => (50_000.0, 220_000.0),
            Demod::Am => (1000.0, 20_000.0),
            Demod::Usb | Demod::Lsb => (500.0, 6000.0),
            Demod::Cw => (50.0, 3000.0),
        }
    }
    /// A width as the host supplied it, made valid: clamped into the range, the default when not finite, `stored` when 0.
    pub fn clamp_width(self, w: f64, stored: f64) -> f64 {
        if w == 0.0 {
            return stored;
        }
        if !w.is_finite() {
            return self.default_width();
        }
        let (lo, hi) = self.width_range();
        w.clamp(lo, hi)
    }
    /// The passband edges relative to the channel frequency for width `w` (Hz).
    pub fn edges(self, w: f64) -> (f64, f64) {
        match self {
            Demod::Usb => (300.0, 300.0 + w),
            Demod::Lsb => (-300.0 - w, -300.0),
            _ => (-w / 2.0, w / 2.0),
        }
    }
    /// A custom width from a dragged edge at `edge_hz` (relative to the channel): the distance to the opposite edge (twice
    /// the distance to the centre for the symmetric filters), clamped and rounded to 10 Hz.
    pub fn width_from_drag(self, edge_hz: f64, current: f64) -> f64 {
        let (lo, hi) = self.edges(current);
        let w = match self {
            Demod::Usb => {
                if (edge_hz - lo).abs() < (edge_hz - hi).abs() {
                    hi - edge_hz
                } else {
                    edge_hz - lo
                }
            }
            Demod::Lsb => {
                if (edge_hz - hi).abs() < (edge_hz - lo).abs() {
                    edge_hz - lo
                } else {
                    hi - edge_hz
                }
            }
            _ => 2.0 * edge_hz.abs(),
        };
        let (a, b) = self.width_range();
        (w.clamp(a, b) / 10.0).round() * 10.0
    }
    /// The default tuning step (Hz).
    pub fn default_step(self) -> f64 {
        match self {
            Demod::Nfm => 12_500.0,
            Demod::Wfm => 50_000.0,
            Demod::Am => 1000.0,
            _ => 100.0,
        }
    }
    /// The step the phone screen picks with the demodulator (Hz).
    pub fn phone_step(self) -> f64 {
        match self {
            Demod::Wfm => 100_000.0,
            Demod::Nfm => 12_500.0,
            Demod::Am => 5000.0,
            Demod::Cw => 100.0,
            Demod::Usb | Demod::Lsb => 1000.0,
        }
    }
    /// The channel filter's tap count.
    pub fn filter_taps(self, w: f64) -> usize {
        if self.is_wide() {
            63
        } else if w < 1500.0 {
            511
        } else {
            255
        }
    }
    /// The voice noise remover is offered.
    pub fn offers_voice_nr(self) -> bool {
        matches!(self, Demod::Nfm | Demod::Wfm | Demod::Am)
    }
}

/// One 10 ms decision of the carrier squelch, timed at the end of its block.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SquelchBlock {
    /// Open (after the hang).
    pub open: bool,
    /// The squelch decision of this block alone, before the 150 ms hang.
    pub wanted: bool,
    /// The receiver state: 1 closed, 2 open.
    pub state: u8,
    /// The smoothed channel level (linear power).
    pub level: f32,
    /// The noise in the channel (linear power). `None` until measured.
    pub noise: Option<f32>,
    /// The threshold in dBFS (-200 for none).
    pub threshold_db: f32,
    /// Blocks with the squelch open and closed so far.
    pub blocks_open: u64,
    /// See `blocks_open`.
    pub blocks_closed: u64,
}

impl Item for SquelchBlock {
    const KIND: &'static str = "general.squelch";
}

/// A noise estimate: noise power in the channel, beside it and inside the passband.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct NoiseEstimate {
    /// The noise beside the channel, as power in the channel's width (linear). `None` when it cannot be measured.
    pub beside: Option<f32>,
    /// The noise inside the passband, as power in the channel's width. `None` when too few bins.
    pub inside: Option<f32>,
    /// Too few bins beside the channel: the noise cannot be measured (Auto squelch is then always open).
    pub impossible: bool,
}

impl Item for NoiseEstimate {
    const KIND: &'static str = "general.noise";
}

/// What the demodulator reports once per report interval.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct DemodReport {
    /// The mean of the FM discriminator over the interval (Hz). 0 for the other demodulators.
    pub fm_offset_hz: f64,
}

impl Item for DemodReport {
    const KIND: &'static str = "general.demod";
}

/// The share of samples a noise blanker blanked, about once a second.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct BlankerReport {
    /// Percent of samples blanked over the last second.
    pub blanked_pct: f32,
    /// The threshold in use (dB).
    pub threshold_db: f32,
}

impl Item for BlankerReport {
    const KIND: &'static str = "general.blanker";
}

/// The voice noise remover's speech probability for the last 10 ms (0 while off).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SpeechProbability(pub f32);

impl Item for SpeechProbability {
    const KIND: &'static str = "general.speech";
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filter_edges() {
        assert_eq!(Demod::Usb.edges(2700.0), (300.0, 3000.0));
        assert_eq!(Demod::Lsb.edges(2700.0), (-3000.0, -300.0));
        assert_eq!(Demod::Nfm.edges(12_500.0), (-6250.0, 6250.0));
        assert_eq!(Demod::Cw.clamp_width(1e6, 500.0), 3000.0);
        assert_eq!(Demod::Cw.clamp_width(0.0, 250.0), 250.0);
        assert_eq!(Demod::Am.clamp_width(f64::NAN, 4000.0), 9000.0);
    }

    #[test]
    fn dragging_an_edge_sets_the_width() {
        assert_eq!(Demod::Nfm.width_from_drag(5003.0, 12_500.0), 10_010.0);
        assert_eq!(Demod::Usb.width_from_drag(2500.0, 2700.0), 2200.0);
        assert_eq!(Demod::Lsb.width_from_drag(-2000.0, 2700.0), 1700.0);
        assert_eq!(Demod::Cw.width_from_drag(1e6, 500.0), 3000.0);
    }
}
