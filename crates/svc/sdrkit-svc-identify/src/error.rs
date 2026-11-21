//! The errors of the identify service.

/// An error of the identify service.
#[derive(Clone, Debug, PartialEq, thiserror::Error)]
pub enum IdentifyError {
    /// No capture is armed, so the samples are not needed.
    #[error("no capture is armed")]
    NotArmed,
    /// The capture holds no samples, or its rate is not a positive number.
    #[error("the capture is empty or has no rate")]
    EmptyCapture,
    /// No stack opens this signal type.
    #[error("no stack opens the signal type {signal}")]
    NoStack {
        /// The key of the signal type.
        signal: String,
    },
}
