//! What the bank decodes: channels at fixed frequencies, and spans that decode every carrier of a raster.

use serde::{Deserialize, Serialize};

/// One channel of the bank: where it is, how wide, and the stack that decodes it in its own session.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ChannelSpec {
    /// The row key of the radio whose stream the channel is cut from, in the engine's radios table.
    pub radio: String,
    /// Its frequency, Hz. It stays when the radio retunes.
    pub freq_hz: f64,
    /// The width that must reach its receiver, Hz.
    pub bandwidth_hz: f64,
    /// The rate that its receiver gets, Hz. 0 takes the least that the stack and the width allow.
    pub rate_hz: f64,
    /// The stack id.
    pub stack: String,
    /// The submode of the stack. Empty for its default.
    pub submode: String,
    /// A name. Empty for the frequency.
    pub label: String,
    /// The signal-to-noise ratio that opens its squelch, dB. `None` keeps it always open.
    pub squelch_db: Option<f64>,
}

/// The rate that a receiver receives: the rate it requests, at least the stack's least rate and 1.25 times the width, at most the
/// capture rate.
pub(crate) fn channel_rate(asked: f64, min_rate: f64, bandwidth: f64, capture_rate: f64) -> f64 {
    asked.max(min_rate).max(1.25 * bandwidth).min(capture_rate)
}

/// The frequencies that a span watches.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Raster {
    /// `count` channels from `first_hz`, `step_hz` apart.
    Grid {
        /// The first channel.
        first_hz: f64,
        /// The spacing.
        step_hz: f64,
        /// How many.
        count: usize,
    },
    /// Listed frequencies.
    List(Vec<f64>),
}

impl Raster {
    /// The frequencies, in order.
    pub fn freqs(&self) -> Vec<f64> {
        match self {
            Raster::Grid {
                first_hz,
                step_hz,
                count,
            } => (0..*count).map(|i| first_hz + i as f64 * step_hz).collect(),
            Raster::List(v) => v.clone(),
        }
    }
}

sdrkit_core::choice_enum! {
    /// When the receivers of a span run.
    #[derive(Serialize, Deserialize)]
    pub enum SpanOpen {
        /// One receiver on every raster channel inside the capture, all the time.
        Always = ("always", "Always"),
        /// A receiver starts on a carrier when it rises above the threshold, strongest first and up to the cap. It stops after the
        /// carrier is quiet for the idle time.
        OnActivity = ("on_activity", "On activity"),
    }
}

/// A span: every carrier of one stack on a raster, each decoded by its own receiver.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SpanSpec {
    /// The row key of the radio whose stream the span reads, in the engine's radios table.
    pub radio: String,
    /// A name for tables and logs.
    pub label: String,
    /// The stack id that decodes each carrier.
    pub stack: String,
    /// The submode of the stack. Empty for its default.
    pub submode: String,
    /// Where the carriers can be.
    pub raster: Raster,
    /// The width of one carrier's channel, Hz.
    pub bandwidth_hz: f64,
    /// The rate that each receiver gets, Hz. 0 takes the least.
    pub rate_hz: f64,
    /// When the receivers run.
    pub open: SpanOpen,
    /// The signal-to-noise ratio that marks a carrier active, dB.
    pub threshold_db: f64,
    /// How long a quiet carrier keeps its receiver, s ([`SpanOpen::OnActivity`]).
    pub idle_s: f64,
    /// The most receivers at once. More active carriers wait, and the strongest go first.
    pub max_receivers: usize,
}
