//! Wide-band sweeps: a range wider than the radio sees at once is cut into hops, the spectra of the hops are stitched into
//! sweeps, and each sweep produces hits, channels, a pseudo-spectrum, a band survey, the hunt for new interference or a health
//! check of the station. A host reads and controls it through [`SweepService`] as an owner. The engine steps the sweep that
//! runs and feeds it spectrum frames.

#![warn(missing_docs)]
#![forbid(unsafe_code)]

mod burst;
mod channels;
mod error;
mod files;
mod floor;
mod grid;
mod health;
mod hits;
mod hunter;
mod machine;
mod plan;
mod profile;
mod record;
mod schedule;
mod service;
mod settle;
mod stitch;
mod survey;
mod traces;

#[cfg(test)]
#[path = "../tests/common/fake.rs"]
mod fake;

pub use channels::{ChannelType, HitAction};
pub use error::SweepError;
pub use health::Level;
pub use hunter::{SourceClass, Trend};
pub use machine::{Input, Phase, RadioInfo, RadioSweepUse, SpectrumFrame, SpectrumNeed, Step};
pub use profile::{Purpose, SweepSetting};
pub use record::{RecordKind, RecordRequest};
pub use service::{
    EndReason, ExportFormat, HitState, SweepCommand, SweepService, ADVICE, BURST, BURSTS, DIFFS,
    DRIFT, FINDINGS, HEALTH_BANDS, HIT, INTERFERENCE, KEY, MAX_PAGE, NEW_INTERFERENCE, PERSISTENT,
    PRESETS, PRESET_BANDS, QUIET, RECORDINGS, ROW, SAVE, SESSION,
};
pub use survey::ChangeKind;

/// Linear power to dB, floored at -200 dB (the floor is kept here, the conversion is the shared one).
#[inline]
pub(crate) fn db(p: f32) -> f32 {
    sdrkit_algo_dsp::meter::power_to_db((p as f64).max(1e-20)) as f32
}

/// dB to linear power.
#[inline]
pub(crate) fn lin(db: f32) -> f32 {
    sdrkit_algo_dsp::meter::db_to_power(db as f64) as f32
}

/// The median of `values`, using `scratch` (no allocation once it has grown). NaN for none.
pub(crate) fn median(values: &[f32], scratch: &mut Vec<f32>) -> f32 {
    sdrkit_algo_dsp::spectrum::percentile(values, 0.5, scratch)
}
