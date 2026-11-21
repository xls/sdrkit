//! The error of a radio: why it could not be opened, started, set or used.

use crate::file::FileFault;
use sdrkit_core::settings::SchemaError;
use sdrkit_core::{NetError, SettingError, SettingId, StageError, StorageError, StorePath};
use sdrkit_graph::stream::SampleType;

sdrkit_core::choice_enum! {
    /// What the other side sent that the driver did not expect.
    pub enum Protocol {
        /// The greeting is not the one of this kind of radio.
        NotThisRadio = ("not_this_radio", "Not this kind of radio"),
        /// A reply is shorter than its header says.
        Short = ("short", "Reply too short"),
        /// A reply does not follow the protocol.
        Malformed = ("malformed", "Reply does not follow the protocol"),
        /// A reply is longer than the driver takes.
        TooLong = ("too_long", "Reply too long"),
        /// The device description cannot be read.
        Description = ("description", "Device description cannot be read"),
        /// The device lacks a part that the driver needs.
        MissingPart = ("missing_part", "Device part missing"),
        /// The server sent a frame the driver does not take.
        UnexpectedFrame = ("unexpected_frame", "Unexpected frame"),
    }
}

/// Why a radio could not be opened, started, set or used.
#[derive(Clone, Debug, PartialEq, thiserror::Error)]
pub enum DriverError {
    /// The link failed.
    #[error("{0}")]
    Net(#[from] NetError),
    /// The address is not one this kind of radio takes.
    #[error("not an address of this kind of radio")]
    Address,
    /// The other side did not follow the protocol.
    #[error("protocol: {0:?}")]
    Protocol(Protocol),
    /// The WebSocket opening handshake failed for a reason that trying again does not mend.
    #[error(transparent)]
    Handshake(crate::ws::UpgradeError),
    /// The radio speaks another version of the protocol.
    #[error("protocol version {got}, the driver needs {need}")]
    Version {
        /// The version the radio reported.
        got: u32,
        /// The lowest version the driver takes.
        need: u32,
    },
    /// Another program uses the radio.
    #[error("the radio is busy")]
    Busy,
    /// The device answered with an error code.
    #[error("the device gave error code {code}")]
    Device {
        /// The code (an errno for the iiod server).
        code: i32,
    },
    /// The device refused the value of a setting.
    #[error("the device refused setting number {}", .id.0)]
    Refused {
        /// The setting.
        id: SettingId,
    },
    /// A setting does not fit the schema.
    #[error(transparent)]
    Setting(#[from] SettingError),
    /// The settings that the open radio adds ([`crate::Source::settings`]) do not make a schema with those of the kind
    /// (a key that is already there, a kind that no setting can have).
    #[error("the settings of the radio: {0}")]
    Schema(SchemaError),
    /// A file in the host's storage could not be read.
    #[error("{path}: {source}")]
    Storage {
        /// The file.
        path: StorePath,
        /// What the storage said.
        source: StorageError,
    },
    /// A file is not a recording the driver can play.
    #[error("{path}: {fault:?}")]
    File {
        /// The file.
        path: StorePath,
        /// What is wrong with it.
        fault: FileFault,
    },
    /// The radio has fewer receivers or outputs than the call names.
    #[error("the radio has {has} streams, {given} were given")]
    Streams {
        /// The streams given or the output named, counted from 1.
        given: usize,
        /// The streams the radio has.
        has: usize,
    },
    /// The ring holds another sample type than the radio provides.
    #[error("the ring holds {ring:?}, the radio gives {radio:?}")]
    WrongSampleType {
        /// The type of the ring.
        ring: SampleType,
        /// The type of the radio ([`crate::Source::sample_type`]).
        radio: SampleType,
    },
    /// The radio cannot transmit.
    #[error("this radio cannot transmit")]
    CannotTransmit,
    /// The call does not fit what the radio does now.
    #[error("not now: the radio is {state:?}")]
    NotNow {
        /// What the radio does.
        state: Doing,
    },
    /// The system refused to start a thread.
    #[error("a thread did not start")]
    Thread,
    /// The error of one driver crate, kept with its type.
    #[error(transparent)]
    Driver(StageError),
}

/// What a radio or a transmitter does, for [`DriverError::NotNow`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Doing {
    /// It streams or plays.
    Running,
    /// It does not stream.
    Stopped,
    /// The transmitter is off air.
    Disarmed,
    /// The transmitter is on air.
    Transmitting,
    /// The transmitter repeats one buffer.
    Cyclic,
    /// It is closed.
    Closed,
}

/// The errno values of the iiod server that can pass: the device is busy, or a transfer failed.
const RETRY_CODES: &[i32] = &[5, 16, 19, 110];

impl DriverError {
    /// The error as text for a person. A refused setting is named by its label and key in `schema`, which must be the
    /// schema that the id of the error counts in (the transmit schema for the error of a transmitter). The `Display`
    /// of the error has only the number of the setting.
    pub fn text(&self, schema: &sdrkit_core::Schema) -> String {
        match self {
            DriverError::Refused { id } => match schema.setting(*id) {
                Some(d) => format!(
                    "the device refused setting \"{}\" ({})",
                    d.field.label, d.field.key
                ),
                None => self.to_string(),
            },
            DriverError::Setting(e) => sdrkit_core::owner::setting_error_text(schema, "setting", e),
            _ => self.to_string(),
        }
    }

    /// True if the same call can work later (the radio is not answering yet, it is busy, the link dropped). False for a
    /// refusal that stays.
    pub fn retryable(&self) -> bool {
        match self {
            DriverError::Net(e) => e.retryable(),
            DriverError::Busy | DriverError::Thread => true,
            DriverError::Device { code } => RETRY_CODES.contains(code),
            DriverError::Storage { source, .. } => {
                matches!(source, StorageError::Io { .. })
            }
            DriverError::Address
            | DriverError::Protocol(_)
            | DriverError::Handshake(_)
            | DriverError::Version { .. }
            | DriverError::Refused { .. }
            | DriverError::Setting(_)
            | DriverError::Schema(_)
            | DriverError::File { .. }
            | DriverError::Streams { .. }
            | DriverError::WrongSampleType { .. }
            | DriverError::CannotTransmit
            | DriverError::NotNow { .. }
            | DriverError::Driver(_) => false,
        }
    }

    /// The error of a driver crate, kept with its type.
    pub fn driver(error: impl std::error::Error + Send + Sync + 'static) -> Self {
        DriverError::Driver(StageError::new(error))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retry_follows_the_kind() {
        assert!(DriverError::Net(NetError::TimedOut).retryable());
        assert!(DriverError::Net(NetError::Refused).retryable());
        assert!(DriverError::Busy.retryable());
        assert!(DriverError::Device { code: 16 }.retryable());
        assert!(DriverError::Device { code: 5 }.retryable());
        assert!(!DriverError::Device { code: 22 }.retryable());
        assert!(!DriverError::Protocol(Protocol::NotThisRadio).retryable());
        assert!(!DriverError::Net(NetError::TlsUnavailable).retryable());
    }
}
