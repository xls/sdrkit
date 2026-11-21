//! Parts of the components of this crate: the `clear` command and the channel stage of a front end.

use sdrkit_core::{Choice, Command, SettingError, Value};
use sdrkit_graph::components::{Channel, ChannelFilter};
use sdrkit_graph::Component;

/// The command that forgets what a component learned of the signal and starts again.
pub const CLEAR: Command =
    Command::new("clear", "Clear").help("Forgets the signal and starts again.");

sdrkit_core::commands! {
    /// The commands of a component that has only [`CLEAR`].
    pub enum ClearCommand {
        /// Forgets the signal and starts again.
        Clear = CLEAR,
    }
}

/// The standard channel component (`dsp.channel`) of a stack's front end: the tuned signal moved to 0 Hz, filtered to
/// `half_width_hz` either side, at exactly `rate_hz`.
///
/// # Errors
///
/// [`SettingError`] if the channel component refuses the width or the rate.
pub fn channel(half_width_hz: f64, rate_hz: f64) -> Result<Channel, SettingError> {
    let mut c = Channel::new(half_width_hz, rate_hz)?;
    let schema = c.info().settings;
    c.apply(schema.id("exact")?, &Value::Bool(true))?;
    c.apply(
        schema.id("filter")?,
        &Value::Choice(ChannelFilter::Wide.index()),
    )?;
    Ok(c)
}
