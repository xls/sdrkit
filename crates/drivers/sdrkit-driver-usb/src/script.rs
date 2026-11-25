//! A script of USB exchanges: what a fake bus replays and what a recording bus writes.

use crate::{Setup, UsbDeviceInfo, UsbError, UsbId};
use sdrkit_util_bits::crc::catalog::CRC32_ISO_HDLC;
use std::time::Duration;

/// How strictly a fake bus holds calls to the order of the steps.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Mode {
    /// Each call must be the next step.
    #[default]
    Strict,
    /// A call takes the first unused step with the same fields after the last one taken, or repeats the reply of the
    /// last step that matched. An output call that matches no step succeeds.
    Lenient,
}

/// The bytes of a transfer, whole or cut short.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Payload {
    /// All bytes.
    Full(Vec<u8>),
    /// The length, the CRC-32/ISO-HDLC of all bytes and the first bytes.
    Cut {
        /// The length of the whole payload.
        len: usize,
        /// The CRC of the whole payload.
        crc: u32,
        /// Its first bytes.
        head: Vec<u8>,
    },
}

impl Payload {
    /// `data` whole if it has at most `keep` bytes, else cut to its first `keep` bytes.
    pub fn of(data: &[u8], keep: usize) -> Payload {
        if data.len() <= keep {
            Payload::Full(data.to_vec())
        } else {
            Payload::Cut {
                len: data.len(),
                crc: CRC32_ISO_HDLC.checksum(data),
                head: data[..keep].to_vec(),
            }
        }
    }

    /// The length of the whole payload.
    pub fn len(&self) -> usize {
        match self {
            Payload::Full(d) => d.len(),
            Payload::Cut { len, .. } => *len,
        }
    }

    /// True for no bytes.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// True if `data` is this payload: equal bytes, or for a cut payload the same length, CRC and head.
    pub fn matches(&self, data: &[u8]) -> bool {
        match self {
            Payload::Full(d) => d == data,
            Payload::Cut { len, crc, head } => {
                data.len() == *len
                    && data.starts_with(head)
                    && CRC32_ISO_HDLC.checksum(data) == *crc
            }
        }
    }

    /// Copies the bytes from `offset` on into `out`, as many as fit; a cut payload gives zero bytes after its
    /// head. Returns the count copied.
    pub fn copy_to(&self, offset: usize, out: &mut [u8]) -> usize {
        let n = self.len().saturating_sub(offset).min(out.len());
        let out = &mut out[..n];
        match self {
            Payload::Full(d) => out.copy_from_slice(&d[offset..offset + n]),
            Payload::Cut { head, .. } => {
                let from_head = head.len().saturating_sub(offset).min(n);
                out[..from_head].copy_from_slice(&head[offset..offset + from_head]);
                out[from_head..].fill(0);
            }
        }
        n
    }
}

/// One call to a device in a script, with its reply.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Step {
    /// [`crate::UsbBus::open`].
    Open {
        /// The device.
        device: UsbId,
        /// The reply.
        reply: Result<(), UsbError>,
    },
    /// [`crate::UsbDevice::claim`].
    Claim {
        /// The device.
        device: UsbId,
        /// The interface number.
        interface: u8,
        /// The reply.
        reply: Result<(), UsbError>,
    },
    /// [`crate::UsbDevice::control_in`].
    ControlIn {
        /// The device.
        device: UsbId,
        /// The setup fields, with the direction bit set.
        setup: Setup,
        /// The count of bytes asked for.
        length: u16,
        /// The bytes the device sent, or the error.
        reply: Result<Vec<u8>, UsbError>,
    },
    /// [`crate::UsbDevice::control_out`].
    ControlOut {
        /// The device.
        device: UsbId,
        /// The setup fields, with the direction bit cleared.
        setup: Setup,
        /// The bytes sent.
        data: Vec<u8>,
        /// The reply.
        reply: Result<(), UsbError>,
    },
    /// [`crate::UsbDevice::bulk_out`].
    BulkOut {
        /// The device.
        device: UsbId,
        /// The endpoint address.
        endpoint: u8,
        /// The bytes sent.
        data: Payload,
        /// The count the device took, or the error.
        reply: Result<usize, UsbError>,
    },
    /// [`crate::UsbDevice::clear_halt`] or [`crate::BulkIn::clear_halt`].
    ClearHalt {
        /// The device.
        device: UsbId,
        /// The endpoint address.
        endpoint: u8,
        /// The reply.
        reply: Result<(), UsbError>,
    },
    /// [`crate::UsbDevice::reset`].
    Reset {
        /// The device.
        device: UsbId,
        /// The reply.
        reply: Result<(), UsbError>,
    },
    /// The device leaves the bus.
    Unplug {
        /// The device.
        device: UsbId,
    },
    /// A device arrives on the bus.
    Plug {
        /// The device.
        info: UsbDeviceInfo,
    },
}

impl Step {
    /// The device the step concerns.
    pub fn device(&self) -> UsbId {
        match self {
            Step::Open { device, .. }
            | Step::Claim { device, .. }
            | Step::ControlIn { device, .. }
            | Step::ControlOut { device, .. }
            | Step::BulkOut { device, .. }
            | Step::ClearHalt { device, .. }
            | Step::Reset { device, .. }
            | Step::Unplug { device } => *device,
            Step::Plug { info } => info.id,
        }
    }

    /// True for a change of the bus (unplug, plug) rather than a call.
    pub fn is_event(&self) -> bool {
        matches!(self, Step::Unplug { .. } | Step::Plug { .. })
    }

    /// True if `call` is the same call as this step: the same kind and request fields, whatever the reply.
    /// The data of a bulk OUT call is compared by [`Payload::matches`].
    pub fn same_call(&self, call: &Step) -> bool {
        match (self, call) {
            (Step::Open { device: a, .. }, Step::Open { device: b, .. }) => a == b,
            (
                Step::Claim {
                    device: a,
                    interface: i,
                    ..
                },
                Step::Claim {
                    device: b,
                    interface: j,
                    ..
                },
            ) => a == b && i == j,
            (
                Step::ControlIn {
                    device: a,
                    setup: s,
                    length: l,
                    ..
                },
                Step::ControlIn {
                    device: b,
                    setup: t,
                    length: m,
                    ..
                },
            ) => a == b && s == t && l == m,
            (
                Step::ControlOut {
                    device: a,
                    setup: s,
                    data: d,
                    ..
                },
                Step::ControlOut {
                    device: b,
                    setup: t,
                    data: e,
                    ..
                },
            ) => a == b && s == t && d == e,
            (
                Step::BulkOut {
                    device: a,
                    endpoint: p,
                    data: d,
                    ..
                },
                Step::BulkOut {
                    device: b,
                    endpoint: q,
                    data: e,
                    ..
                },
            ) => {
                a == b
                    && p == q
                    && match e {
                        Payload::Full(bytes) => d.matches(bytes),
                        cut => d == cut,
                    }
            }
            (
                Step::ClearHalt {
                    device: a,
                    endpoint: p,
                    ..
                },
                Step::ClearHalt {
                    device: b,
                    endpoint: q,
                    ..
                },
            ) => a == b && p == q,
            (Step::Reset { device: a, .. }, Step::Reset { device: b, .. }) => a == b,
            (Step::Unplug { device: a }, Step::Unplug { device: b }) => a == b,
            (Step::Plug { info: a }, Step::Plug { info: b }) => a == b,
            _ => false,
        }
    }
}

/// What one bulk IN item gives.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BulkKind {
    /// These bytes.
    Data(Payload),
    /// This error from [`crate::BulkIn::next`].
    Error(UsbError),
    /// The device leaves the bus.
    Unplug,
}

/// One item of the bulk IN data of an endpoint.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BulkItem {
    /// The device.
    pub device: UsbId,
    /// The endpoint address.
    pub endpoint: u8,
    /// The time since the previous item of the endpoint (or the start of the stream) before this one is ready.
    pub after: Duration,
    /// How many times the item is given, at least 1.
    pub repeat: u32,
    /// What it gives.
    pub kind: BulkKind,
}

impl BulkItem {
    /// `data` from `endpoint` of `device`, once and at once.
    pub fn data(device: UsbId, endpoint: u8, data: Vec<u8>) -> BulkItem {
        BulkItem {
            device,
            endpoint,
            after: Duration::ZERO,
            repeat: 1,
            kind: BulkKind::Data(Payload::Full(data)),
        }
    }
}

/// One line of a script.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Entry {
    /// A call or a change of the bus, in the order of the calls.
    Step(Step),
    /// Bulk IN data, in the order of its endpoint.
    Bulk(BulkItem),
}

/// The devices at the start and the exchanges after it. See the crate documentation for the text form.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Script {
    /// How strictly a fake bus holds calls to the steps.
    pub mode: Mode,
    /// The devices attached at the start.
    pub devices: Vec<UsbDeviceInfo>,
    /// The steps and bulk items in the order they were written.
    pub entries: Vec<Entry>,
}

impl Script {
    /// An empty strict script.
    pub fn new() -> Script {
        Script::default()
    }

    /// Adds a step.
    pub fn step(&mut self, step: Step) -> &mut Script {
        self.entries.push(Entry::Step(step));
        self
    }

    /// Adds a bulk item.
    pub fn bulk(&mut self, item: BulkItem) -> &mut Script {
        self.entries.push(Entry::Bulk(item));
        self
    }

    /// The steps in order, each with its index among the entries.
    pub fn steps(&self) -> impl Iterator<Item = (usize, &Step)> {
        self.entries
            .iter()
            .enumerate()
            .filter_map(|(i, e)| match e {
                Entry::Step(s) => Some((i, s)),
                Entry::Bulk(_) => None,
            })
    }

    /// Reads the script file at `path`.
    ///
    /// # Errors
    ///
    /// When the file cannot be read or is not a script.
    pub fn load(path: &std::path::Path) -> Result<Script, crate::ParseError> {
        let text = std::fs::read_to_string(path).map_err(|e| crate::ParseError {
            line: 0,
            text: format!("{}: {e}", path.display()),
        })?;
        Script::parse(&text)
    }

    /// Writes the script to the file at `path`.
    ///
    /// # Errors
    ///
    /// When the file cannot be written.
    pub fn save(&self, path: &std::path::Path) -> std::io::Result<()> {
        std::fs::write(path, self.to_text())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cut_payload_matches_its_whole() {
        let data: Vec<u8> = (0..100u8).collect();
        let p = Payload::of(&data, 8);
        assert_eq!(p.len(), 100);
        assert!(matches!(p, Payload::Cut { .. }));
        assert!(p.matches(&data));
        let mut other = data.clone();
        other[50] ^= 1;
        assert!(!p.matches(&other));
        let mut out = [0xffu8; 12];
        assert_eq!(p.copy_to(4, &mut out), 12);
        assert_eq!(&out[..4], &[4, 5, 6, 7]);
        assert!(out[4..].iter().all(|&b| b == 0));
        assert_eq!(Payload::of(&data, 100), Payload::Full(data));
    }
}
