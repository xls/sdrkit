//! Synchronisation and linear-modulation components: the chain from a channel to soft bits (matched filter, symbol sync,
//! carrier sync, slicer), and the measurements beside it. A `Retuned` tag resets every loop, search and estimate at its item.

mod burst;
mod carrier_sync;
mod equaliser;
mod freq_search;
mod matched;
pub mod points;
mod slicer;
mod snr;
mod squelch;
mod symbol_sync;

pub use burst::{BurstDetect, IqBurst};
pub use carrier_sync::CarrierSyncComponent;
pub use equaliser::Equaliser;
pub use freq_search::{FreqEstimate, FreqMethod, FreqSearch};
pub use matched::{MatchedFilter, MatchedFilterSetting};
pub use slicer::Slicer;
pub use snr::SnrMeter;
pub use squelch::{SquelchComponent, SquelchKind};
pub use symbol_sync::SymbolSyncComponent;

use sdrkit_algo_dsp::pulse::Shape;
use sdrkit_algo_dsp::DspError;
use sdrkit_core::ConfigError;
use sdrkit_graph::{Buffer, Registry, TagKind};

/// Adds these components.
pub fn register(r: &mut Registry) {
    r.add(
        "modem.matched_filter",
        || Box::new(MatchedFilter::default()),
    );
    r.add("modem.symbol_sync", || {
        Box::new(SymbolSyncComponent::default())
    });
    r.add("modem.carrier_sync", || {
        Box::new(CarrierSyncComponent::default())
    });
    r.add("modem.freq_search", || Box::new(FreqSearch::default()));
    r.add("modem.slicer", || Box::new(Slicer::default()));
    r.add("modem.equaliser", || Box::new(Equaliser::default()));
    r.add("modem.snr", || Box::new(SnrMeter::default()));
    r.add("modem.squelch", || Box::new(SquelchComponent::default()));
    r.add("modem.squelch_real", || {
        Box::new(SquelchComponent::on_real())
    });
    r.add("modem.burst_detect", || Box::new(BurstDetect::default()));
}

sdrkit_core::choice_enum! {
    /// A pulse shape of the matched filter and the symbol sync.
    pub enum PulseShape {
        /// Root raised cosine, roll-off `beta`.
        Rrc = ("rrc", "Root raised cosine"),
        /// Raised cosine, roll-off `beta`.
        Rc = ("rc", "Raised cosine"),
        /// Gaussian, bandwidth-time product `bt`.
        Gaussian = ("gaussian", "Gaussian"),
        /// Half sine.
        HalfSine = ("half_sine", "Half sine"),
        /// Rectangle.
        Rect = ("rect", "Rectangle"),
    }
}

/// The shape `p` with roll-off `beta` (RRC, RC) or bandwidth-time product `bt` (Gaussian).
pub(crate) fn shape(p: PulseShape, beta: f64, bt: f64) -> Result<Shape, DspError> {
    let s = match p {
        PulseShape::Rrc => Shape::Rrc { beta },
        PulseShape::Rc => Shape::Rc { beta },
        PulseShape::Gaussian => Shape::Gaussian { bt },
        PulseShape::HalfSine => Shape::HalfSine,
        PulseShape::Rect => Shape::Rect,
    };
    s.check()?;
    Ok(s)
}

/// [`sdrkit_graph::walk_tags`] with one state for both callbacks: `run(state, a, b)` processes items `a..b` of the call, and
/// `tag(state, t)` handles each tag after the items before it.
pub(crate) fn split_at_tags<S>(
    state: &mut S,
    input: &Buffer,
    len: usize,
    mut run: impl FnMut(&mut S, usize, usize),
    mut tag: impl FnMut(&mut S, &sdrkit_graph::Tag),
) {
    sdrkit_graph::walk_tags(input.first(), len, input.tags(), |step| match step {
        sdrkit_graph::TagStep::Items(a, b) => run(state, a, b),
        sdrkit_graph::TagStep::Tag(_, t) => tag(state, t),
    });
}

/// Whether a tag resets loops and searches.
pub(crate) fn is_retune(t: &sdrkit_graph::Tag) -> bool {
    matches!(t.kind, TagKind::Retuned { .. })
}

/// `x` rounded to two decimals.
pub(crate) fn r2(x: f64) -> f64 {
    (x * 100.0).round() / 100.0
}

/// `Ok` if `rate` is a usable rate (positive and finite), else [`ConfigError::NoRate`].
pub(crate) fn need_rate(rate: f64) -> Result<(), ConfigError> {
    if rate > 0.0 && rate.is_finite() {
        Ok(())
    } else {
        Err(ConfigError::NoRate)
    }
}
