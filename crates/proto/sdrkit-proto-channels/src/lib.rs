//! Channelised voice reception: many narrow channels inside one capture. A [`Finder`] tracks the carriers on a raster, a
//! [`Player`] chooses the channel that plays and runs the scan, and a [`CallBook`] keeps the calls and the statistics of each
//! channel. A stack supplies the raster, the thresholds and its channel plan, and wraps each part in a component of its own.

#![deny(missing_docs)]
#![forbid(unsafe_code)]

mod calls;
mod clock;
mod finder;
mod items;
mod player;

pub use calls::{Call, CallBook, CallStep, ChannelStats};
pub use clock::SignalClock;
pub use finder::{Finder, FinderConfig};
pub use items::{Blocks, Carrier, Carriers, Chan, ChanBlock, PlayState, ScanState};
pub use player::{Player, ScanPlan};

/// The audio rate of a channel, Hz.
pub const AUDIO_RATE: f64 = 48_000.0;
/// A squelch block: 10 ms of audio.
pub const BLOCK: usize = 480;
