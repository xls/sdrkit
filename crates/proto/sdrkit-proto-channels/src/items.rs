//! The items the parts exchange.

use sdrkit_graph::Item;

/// A carrier the finder tracks.
#[derive(Clone, Debug, PartialEq)]
pub struct Carrier {
    /// Its number, kept while it is tracked.
    pub id: u64,
    /// Absolute frequency of its raster cell, Hz.
    pub freq_hz: f64,
    /// The offset of the cell in the stream, Hz.
    pub offset_hz: f64,
    /// Its level, dBFS.
    pub level_dbfs: f64,
    /// Its level over the cell's noise floor, dB.
    pub snr_db: f64,
    /// It is above the threshold now.
    pub active: bool,
    /// The power centroid of the signal, absolute, Hz.
    pub centroid_hz: f64,
    /// The occupied width: the spectrum more than 6 dB above the floor, smoothed, Hz.
    pub width_hz: f64,
    /// Signal time since it was first reported, s.
    pub age_s: f64,
}

/// The carriers of one estimate.
#[derive(Clone, Debug, Default)]
pub struct Carriers(pub Vec<Carrier>);
impl Item for Carriers {
    const KIND: &'static str = "channels.carriers";
}

/// A channel: the key the stack gave it and its frequency.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Chan {
    /// The key: one value per channel of the stack.
    pub key: i64,
    /// Where to listen, Hz.
    pub hz: f64,
}

/// One 10 ms block of one channel.
#[derive(Clone, Debug)]
pub struct ChanBlock {
    /// The channel key.
    pub key: i64,
    /// The channel frequency, Hz.
    pub freq_hz: f64,
    /// The signal time of the block, s.
    pub time: f64,
    /// The squelch is open.
    pub open: bool,
    /// The channel level, dBFS.
    pub level_dbfs: f64,
    /// The squelch measure, dB.
    pub measure_db: f64,
    /// The gated audio of the block, 48 kHz.
    pub audio: Vec<f32>,
}

/// The blocks of one pass.
#[derive(Clone, Debug, Default)]
pub struct Blocks(pub Vec<ChanBlock>);
impl Item for Blocks {
    const KIND: &'static str = "channels.blocks";
}

sdrkit_core::choice_enum! {
    /// What the player does.
    pub enum ScanState {
        /// Plays what it chose, or nothing.
        Idle = ("idle", "Idle"),
        /// Holds the priority channel.
        Priority = ("priority", "Priority channel"),
        /// Looks at the priority channel.
        PriorityProbe = ("priority_probe", "Looking at the priority channel"),
        /// Steps through the scan list.
        Scanning = ("scanning", "Scanning"),
        /// Holds an active channel of the scan list.
        Holding = ("holding", "Holding"),
    }
}

impl ScanState {
    /// The state in words, for the reason of a retune request.
    pub fn words(self) -> &'static str {
        match self {
            ScanState::Idle => "idle",
            ScanState::Priority => "priority",
            ScanState::PriorityProbe => "priority probe",
            ScanState::Scanning => "scanning",
            ScanState::Holding => "holding",
        }
    }
}

/// What plays, fed back to the finder and given to the log.
#[derive(Clone, Debug)]
pub struct PlayState {
    /// The channel that plays.
    pub playing: Option<Chan>,
    /// What the player does.
    pub scan_state: ScanState,
    /// The frequency the single-tuner scan requires the radio on, Hz.
    pub tune_hz: Option<f64>,
}
impl Item for PlayState {
    const KIND: &'static str = "channels.play";
}

impl Default for PlayState {
    fn default() -> Self {
        PlayState {
            playing: None,
            scan_state: ScanState::Idle,
            tune_hz: None,
        }
    }
}
