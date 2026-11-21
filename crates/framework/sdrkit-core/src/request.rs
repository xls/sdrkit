//! Requests from a receiver or a service to the host: retune, hop through a list of channels, follow a channel for a while,
//! stop following, tune a session to a stored channel, save a channel as a memory, or record a channel.
//!
//! A receiver never tunes the radio itself. It puts a [`HostRequest`] in [`Output::requests`](crate::Output::requests) (a graph does
//! it from a graph output of typed `HostRequest` items). The host grants the request, refuses it (the user locked the
//! frequency), or passes it to a second receiver. A service holds no session and sends its requests to the engine in the same form.

use crate::owner::OwnerKey;
use crate::signal::SignalType;
use crate::value::Value;
use crate::Hz;
use serde::{Deserialize, Serialize};

/// A stored value of a setting of another owner: the owner key, the setting key and the typed value. A host reads the label
/// and the choices of the setting through that owner's schema.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OwnerSetting {
    /// The owner of the setting (`"stack.general"`).
    pub owner: OwnerKey,
    /// The key of the setting in the owner's schema.
    pub key: String,
    /// The value.
    pub value: Value,
}

/// What a tuned session receives with.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "target", content = "value", rename_all = "snake_case")]
pub enum TuneTarget {
    /// The stack that runs now.
    Running,
    /// The stack with this id (`"fm"`), from a stored channel.
    Stack(String),
    /// The stack that the engine maps to this signal type.
    Signal(SignalType),
}

impl TuneTarget {
    /// The target of a stored stack id: the stack with that id, or the running stack when the id is empty.
    pub fn of_stack(id: &str) -> TuneTarget {
        if id.is_empty() {
            TuneTarget::Running
        } else {
            TuneTarget::Stack(id.to_owned())
        }
    }
}

/// One request from a receiver to the host.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "request", rename_all = "snake_case")]
pub enum HostRequest {
    /// Tune to `tuned_hz` (the receiver's own frequency moves, and the radio can stay where it is when the new frequency fits in the
    /// capture).
    Retune {
        /// Where to tune.
        tuned_hz: Hz,
        /// Why, for the log ("control channel moved").
        reason: String,
    },
    /// Visit these frequencies in turn, `dwell_s` on each, until a lock or a [`HostRequest::Retune`] (HFDL ground stations, DSC
    /// watch channels, a band survey).
    Hop {
        /// The frequencies, in order.
        channels: Vec<Hz>,
        /// Seconds on each.
        dwell_s: f64,
        /// Stay on a channel where the receiver reports lock.
        stop_on_lock: bool,
    },
    /// Follow a channel announced on this one (a trunked voice call, a data channel): decode `freq_hz` for up to `max_s`
    /// seconds, in a second receiver when the capture holds both, else by retuning and coming back.
    Follow {
        /// The channel.
        freq_hz: Hz,
        /// What the channel carries. `None` decodes it with this receiver's stack.
        signal: Option<SignalType>,
        /// The talkgroup of the call. `None` when the channel is not a group call.
        talkgroup: Option<u32>,
        /// Its bandwidth, Hz.
        bandwidth_hz: Hz,
        /// What it is, for people ("Fire dispatch").
        label: String,
        /// The longest it is followed, seconds (the receiver sends [`HostRequest::Release`] sooner when the call ends).
        max_s: f64,
        /// Settings of the stack that decodes the channel, by key. A key that the stack does not have, or a value that
        /// it refuses, is left out.
        settings: Vec<(String, Value)>,
    },
    /// Stop following `freq_hz` (the call ended); the host goes back to this receiver's own channel.
    Release {
        /// The channel followed.
        freq_hz: Hz,
    },
    /// Tune session `session` to a channel (a memory, a preset, a database entry, a signal that a service found): start the
    /// stack of `target` if it is not the running one, apply `settings`, then tune to `freq_hz`.
    Tune {
        /// The row key of the session in the engine's sessions table. Empty in a request of a session for itself.
        session: String,
        /// Where to tune. `None` keeps the frequency.
        freq_hz: Option<Hz>,
        /// What to receive with.
        target: TuneTarget,
        /// The submode of the stack. Empty for its default.
        submode: String,
        /// The sample rate of the radio. Only the stack of the session sets it, in a request for itself: its operator
        /// demands the rate from then on. `None` keeps it.
        rate_hz: Option<Hz>,
        /// The channel bandwidth. `None` keeps it.
        bandwidth_hz: Option<Hz>,
        /// The settings to apply first, each with its owner.
        settings: Vec<OwnerSetting>,
    },
    /// Save a channel as a memory.
    StoreMemory {
        /// The name of the memory, for people.
        name: String,
        /// The channel.
        freq_hz: Hz,
        /// What to receive it with.
        target: TuneTarget,
        /// The channel bandwidth. `None` for the stack's own.
        bandwidth_hz: Option<Hz>,
        /// The settings that the memory keeps, each with its owner.
        settings: Vec<OwnerSetting>,
        /// The group of the memory. Empty for none.
        group: String,
    },
    /// Count a hit on a memory: a scan stopped on its channel.
    MemoryHit {
        /// The row key of the memory in the memories table of the memory service.
        memory: String,
        /// When the hit started, seconds of wall time.
        at_s: f64,
        /// The tone heard on the channel. Empty for none.
        tone: String,
    },
    /// Record a channel.
    Record {
        /// The channel.
        freq_hz: Hz,
        /// Its bandwidth, Hz.
        bandwidth_hz: Hz,
        /// The longest recording, seconds. `None` records until it is stopped.
        max_s: Option<f64>,
        /// What it is, for people. The recording carries it.
        label: String,
    },
}

impl HostRequest {
    /// A retune to `tuned_hz`.
    pub fn retune(tuned_hz: Hz, reason: &str) -> HostRequest {
        HostRequest::Retune {
            tuned_hz,
            reason: reason.into(),
        }
    }
    /// Makes sure that the request can be carried out: finite, positive frequencies and rates, at least one channel to hop, and
    /// a dwell and a maximum time above 0.
    ///
    /// # Errors
    ///
    /// [`RequestError`] for the first bad part.
    pub fn validate(&self) -> Result<(), RequestError> {
        let hz = |f: Hz| {
            if f.is_finite() && f > 0.0 {
                Ok(())
            } else {
                Err(RequestError::NotAFrequency { got: f })
            }
        };
        let time = |t: f64| {
            if t.is_finite() && t > 0.0 {
                Ok(())
            } else {
                Err(RequestError::NotATime { got: t })
            }
        };
        match self {
            HostRequest::Retune { tuned_hz, .. } => hz(*tuned_hz),
            HostRequest::Hop {
                channels, dwell_s, ..
            } => {
                if channels.is_empty() {
                    return Err(RequestError::NoChannels);
                }
                time(*dwell_s)?;
                channels.iter().try_for_each(|&f| hz(f))
            }
            HostRequest::Follow {
                freq_hz,
                bandwidth_hz,
                max_s,
                ..
            } => {
                hz(*freq_hz)?;
                hz(*bandwidth_hz)?;
                time(*max_s)
            }
            HostRequest::Release { freq_hz } => hz(*freq_hz),
            HostRequest::Tune {
                freq_hz,
                rate_hz,
                bandwidth_hz,
                ..
            } => [freq_hz, rate_hz, bandwidth_hz]
                .into_iter()
                .flatten()
                .try_for_each(|&f| hz(f)),
            HostRequest::StoreMemory {
                freq_hz,
                bandwidth_hz,
                ..
            } => {
                hz(*freq_hz)?;
                bandwidth_hz.map_or(Ok(()), hz)
            }
            HostRequest::Record {
                freq_hz,
                bandwidth_hz,
                max_s,
                ..
            } => {
                hz(*freq_hz)?;
                hz(*bandwidth_hz)?;
                max_s.map_or(Ok(()), time)
            }
            HostRequest::MemoryHit { at_s, .. } => time(*at_s),
        }
    }
}

/// Why a [`HostRequest`] was refused.
#[derive(Clone, Copy, Debug, PartialEq, thiserror::Error)]
pub enum RequestError {
    /// A frequency or a bandwidth is not finite or not above 0.
    #[error("{got} Hz is not a frequency")]
    NotAFrequency {
        /// The value given, Hz.
        got: Hz,
    },
    /// A dwell or a maximum time is not finite or not above 0.
    #[error("{got} s is not a time above 0")]
    NotATime {
        /// The value given, seconds.
        got: f64,
    },
    /// A hop has no channels.
    #[error("a hop needs at least one channel")]
    NoChannels,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requests_check_and_serialise() {
        let r = HostRequest::retune(129_125_000.0, "control channel moved");
        assert_eq!(r.validate(), Ok(()));
        let j = serde_json::to_value(&r).unwrap();
        assert_eq!(j["request"], "retune");
        assert!(HostRequest::retune(f64::NAN, "").validate().is_err());
        let hop = HostRequest::Hop {
            channels: vec![],
            dwell_s: 30.0,
            stop_on_lock: true,
        };
        assert_eq!(hop.validate(), Err(RequestError::NoChannels));
        let f = HostRequest::Follow {
            freq_hz: 451e6,
            signal: Some(SignalType::Dmr),
            talkgroup: Some(1201),
            bandwidth_hz: 12.5e3,
            label: "Fire dispatch".into(),
            max_s: 0.0,
            settings: Vec::new(),
        };
        assert_eq!(f.validate(), Err(RequestError::NotATime { got: 0.0 }));
        let j = serde_json::to_value(&f).unwrap();
        assert_eq!(j["signal"], "dmr");
        assert_eq!(serde_json::from_value::<HostRequest>(j).unwrap(), f);
        assert_eq!(HostRequest::Release { freq_hz: 451e6 }.validate(), Ok(()));
        let tune = |rate_hz| HostRequest::Tune {
            session: "1".into(),
            freq_hz: Some(118.1e6),
            target: TuneTarget::Stack("general".into()),
            submode: String::new(),
            rate_hz,
            bandwidth_hz: None,
            settings: vec![OwnerSetting {
                owner: OwnerKey::of("stack.general"),
                key: "width".into(),
                value: Value::F64(8000.0),
            }],
        };
        assert_eq!(tune(None).validate(), Ok(()));
        assert_eq!(
            tune(Some(-1.0)).validate(),
            Err(RequestError::NotAFrequency { got: -1.0 })
        );
        let store = HostRequest::StoreMemory {
            name: "Tower".into(),
            freq_hz: 118.1e6,
            target: TuneTarget::Signal(SignalType::AmVoice),
            bandwidth_hz: Some(0.0),
            settings: vec![],
            group: String::new(),
        };
        assert_eq!(
            store.validate(),
            Err(RequestError::NotAFrequency { got: 0.0 })
        );
        let record = HostRequest::Record {
            freq_hz: 118.1e6,
            bandwidth_hz: 8e3,
            max_s: Some(-2.0),
            label: String::new(),
        };
        assert_eq!(record.validate(), Err(RequestError::NotATime { got: -2.0 }));
    }
}
