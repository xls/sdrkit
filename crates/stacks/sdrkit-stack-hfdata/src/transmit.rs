//! What the transmitters of the crate (`ale`, `hfbeacons`) share: the state of the session's transmitter, the stated start
//! of a burst, the end mark, and the reasons a transmit command is refused.

use sdrkit_components::tx::{end_mark, BurstTiming};
use sdrkit_graph::{Format, Io, RunContext};

/// The key-down level of every transmitter of the crate, full scale 1 (-6 dBFS).
pub const LEVEL: f32 = 0.5;
/// The lowest session rate a transmitter accepts.
pub const MIN_RATE: f64 = 12_000.0;

/// Why a transmit command was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TxError {
    /// The engine's transmit switch is off.
    Off,
    /// A transmission is under way.
    Busy,
    /// The session's rate is below [`MIN_RATE`] or unknown.
    Rate,
    /// A setting that the transmission needs is empty: its key.
    Empty(&'static str),
    /// An argument or setting holds a character or a length that the signal cannot carry: its key.
    Invalid(&'static str),
    /// A message names no station and no station is linked.
    NoLink,
}

impl std::fmt::Display for TxError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TxError::Off => f.write_str("the transmitter is off"),
            TxError::Busy => f.write_str("a transmission is under way"),
            TxError::Rate => f.write_str("the sample rate is below 12 ksps or not known yet"),
            TxError::Empty(k) => write!(f, "the setting {k} is empty"),
            TxError::Invalid(k) => {
                write!(f, "{k} holds characters or a length that cannot be sent")
            }
            TxError::NoLink => f.write_str("no station is linked; name the called address"),
        }
    }
}

impl std::error::Error for TxError {}

/// The session's transmitter as one transmitter component sees it: the rate and dial of its input, the transmit switch,
/// the timing of a stated start, and an end mark owed by a cut burst.
#[derive(Default)]
pub struct Air {
    /// The session's rate, Hz.
    pub rate: f64,
    /// The dial, Hz.
    pub dial: f64,
    /// The engine's transmit switch.
    pub transmit: bool,
    timing: BurstTiming,
    end_owed: bool,
}

impl Air {
    /// Takes the rate and dial of the input format `f`; returns the format of the output: complex baseband centred on
    /// the dial.
    pub fn configure(&mut self, f: &Format) -> Format {
        self.rate = f.rate;
        self.dial = if f.tuned_hz > 0.0 {
            f.tuned_hz
        } else {
            f.centre_hz
        };
        Format::iq(self.rate, self.dial, self.dial)
    }

    /// Takes the transmit switch, the clock and the host of `ctx`.
    pub fn attach(&mut self, ctx: &RunContext) {
        self.transmit = ctx.transmit;
        self.timing.attach(ctx);
    }

    /// Refuses a new transmission while the switch is off or the rate is too low.
    ///
    /// # Errors
    ///
    /// [`TxError::Off`] or [`TxError::Rate`].
    pub fn ready(&self) -> Result<(), TxError> {
        if !self.transmit {
            Err(TxError::Off)
        } else if self.rate < MIN_RATE {
            Err(TxError::Rate)
        } else {
            Ok(())
        }
    }

    /// The time on air of a burst whose first sample is written in a pass of `pass` samples ([`BurstTiming::start_time`]).
    pub fn start_time(&self, pass: usize) -> Option<f64> {
        self.timing.start_time(pass as f64 / self.rate.max(1.0))
    }

    /// Notes that a burst was cut: its end mark goes out at the start of the next pass.
    pub fn owe_end(&mut self) {
        self.end_owed = true;
    }

    /// Writes the end mark owed by a cut burst, on one silent sample.
    pub fn settle(&mut self, io: &mut Io) {
        if std::mem::take(&mut self.end_owed) {
            end_mark(io, 0);
        }
    }
}
