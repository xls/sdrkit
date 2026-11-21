//! The scan machine that the engine steps: it takes the time and an observation of the tuned channel, and answers with a step.

use crate::observe::Observation;
use sdrkit_core::{OwnerSetting, Results};

/// An action a scanner requests from the engine.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    /// Tune to the channel that [`ScanMachine::target`] returns: start its stack if it is not the running one, apply its
    /// settings, then tune. The results of the receiver start empty.
    Tune,
    /// The scan is over. The engine tunes back to where it was when the scan started.
    Done,
}

/// The channel to tune, borrowed from the scanner.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Target<'a> {
    /// The frequency, Hz.
    pub freq_hz: f64,
    /// The stack id. Empty keeps the running stack.
    pub stack: &'a str,
    /// The submode of the stack. Empty for its default.
    pub submode: &'a str,
    /// The name of the channel. Empty for a step of a band.
    pub name: &'a str,
    /// The settings to apply before the tune.
    pub settings: &'a [OwnerSetting],
    /// The channel bandwidth, Hz. `None` keeps the bandwidth of the stack.
    pub bandwidth_hz: Option<f64>,
    /// The sample rate of the radio for this tune, Hz: the widest rate, while a survey looks at a block without the
    /// stack. `None` gives the stack the rate that it asks for.
    pub rate_hz: Option<f64>,
}

sdrkit_core::choice_enum! {
    /// Where a scan is.
    pub enum ScanState {
        /// No scan has run.
        Idle = ("idle", "Idle"),
        /// After a retune, before the channel is judged.
        Settle = ("settle", "Settle"),
        /// Listening to a quiet channel.
        Listen = ("listen", "Listen"),
        /// Holding an active channel.
        Hold = ("hold", "Hold"),
        /// Waiting after activity ended.
        Hang = ("hang", "Hang"),
        /// Held by the user.
        Paused = ("paused", "Paused"),
        /// Testing a channel for a lock.
        Test = ("test", "Test"),
        /// The scan ended.
        Done = ("done", "Done"),
        /// A find holds the scan.
        Locked = ("locked", "Locked"),
        /// A survey looks at a wide block for candidates.
        Look = ("look", "Look"),
    }
}

sdrkit_core::choice_enum! {
    /// Why a scan ended.
    pub enum EndReason {
        /// The scan has not ended.
        None = ("none", "Not ended"),
        /// Every pass is done.
        PassesDone = ("passes_done", "Every pass done"),
        /// Every channel is locked out.
        AllLockedOut = ("all_locked_out", "Every channel locked out"),
        /// There was nothing to scan.
        NothingToScan = ("nothing_to_scan", "Nothing to scan"),
        /// The scan was stopped.
        Stopped = ("stopped", "Stopped"),
        /// Every channel was tested.
        Complete = ("complete", "Complete"),
        /// The total time of the scan is over.
        TimeOver = ("time_over", "Total time over"),
    }
}

/// Where a scan is, for the results of the service.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Progress {
    /// The state.
    pub state: ScanState,
    /// The channel tuned now.
    pub index: usize,
    /// The number of channels.
    pub count: usize,
    /// The scan is at the priority channel for a check.
    pub at_priority: bool,
    /// The passes done.
    pub pass: u32,
    /// The retunes done.
    pub steps: u64,
    /// The channels with a station (a channel scan).
    pub found: usize,
    /// Why the scan ended.
    pub end: EndReason,
}

/// A spectrum of the samples that the tuned session receives: more channels than the tuned one show in it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spectrum<'a> {
    /// The frequency at the middle bin, Hz.
    pub centre_hz: f64,
    /// The width of the spectrum, Hz.
    pub span_hz: f64,
    /// The power of each bin, dB, lowest frequency first.
    pub power_db: &'a [f32],
}

/// A scanner, driven by time and observations. It never waits and never touches a radio.
pub trait ScanMachine: Send {
    /// A spectrum of the samples of the tuned session came at `now_s`. A scanner that visits channels uses it to leave
    /// out the channels that show no signal.
    fn spectrum(&mut self, now_s: f64, spectrum: &Spectrum<'_>) {
        let _ = (now_s, spectrum);
    }
    /// Starts at `now_s`, seconds on the clock of the caller (only differences matter). Returns the first step.
    fn start(&mut self, now_s: f64) -> Step;
    /// Time has passed and the channel was observed. `results` are the results of the tuned stack, if the caller has them.
    /// Returns a step if the scanner requires an action.
    fn tick(&mut self, now_s: f64, obs: &Observation, results: Option<&Results>) -> Option<Step>;
    /// The channel of the last [`Step::Tune`].
    fn target(&self) -> Option<Target<'_>>;
    /// True if sound must play now (a stop is held).
    fn audio_wanted(&self) -> bool;
    /// True if the scan has ended.
    fn finished(&self) -> bool;
    /// Where the scan is.
    fn progress(&self) -> Progress;
}
