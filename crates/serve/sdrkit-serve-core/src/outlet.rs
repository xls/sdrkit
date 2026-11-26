//! Where a client's stream goes: an [`Outlet`] takes each block in the format it states, with a [`FrameInfo`] (sequence,
//! sample index, time, gap), and frames and sends it as its protocol wants. [`ConnOutlet`] writes to the client's own
//! connection, bare or with a header per packet from a [`Framer`].

use crate::hub::Gap;
use sdrkit_core::net::{timed_out, Conn};
use sdrkit_driver_core::sample::size;
use sdrkit_graph::stream::SampleType;
use std::io::ErrorKind;

/// What a block of samples is, for the framing of a protocol.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FrameInfo {
    /// The hub's chunk counter of the block.
    pub seq: u64,
    /// The index of the block's first sample in the source, lost samples counted.
    pub first_sample: u64,
    /// The time of the first sample from the start of the hub, ns: `first_sample / rate`.
    pub time_ns: i64,
    /// The rate of the source, Hz.
    pub rate_hz: f64,
    /// The centre of the source, Hz.
    pub centre_hz: f64,
    /// What came before the block for this client: its own drops and the losses of the source as [`Gap::Lost`], or a
    /// retune.
    pub gap: Gap,
}

/// Why an outlet stopped sending.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutletError {
    /// The client or the link is gone.
    Closed,
    /// The `stop` check said to stop while the outlet waited.
    Stopped,
}

/// The end of a client's stream.
pub trait Outlet: Send {
    /// The sample type the outlet takes: Cu8, Cs8, Cs16 or Cf32.
    fn format(&self) -> SampleType;
    /// Sends `samples` (whole samples of [`Outlet::format`]) described by `info`. While it waits it calls `stop`, and
    /// returns [`OutletError::Stopped`] when that is true.
    ///
    /// # Errors
    ///
    /// [`OutletError`] if the samples could not all be sent.
    fn send(
        &mut self,
        samples: &[u8],
        info: &FrameInfo,
        stop: &mut dyn FnMut() -> bool,
    ) -> Result<(), OutletError>;
}

/// The header that a protocol puts before each packet of a stream on a connection.
pub trait Framer: Send {
    /// The most payload bytes in one packet.
    fn max_payload(&self) -> usize;
    /// Appends the header of a packet of `payload` described by `info` to `out`.
    fn header(&mut self, payload: &[u8], info: &FrameInfo, out: &mut Vec<u8>);
}

/// Writes all of `bytes` to `conn`. A write that times out is tried again until `stop` says to stop.
///
/// # Errors
///
/// [`OutletError::Closed`] if the connection fails, [`OutletError::Stopped`] if `stop` said to stop.
pub fn write_patient(
    conn: &mut dyn Conn,
    mut bytes: &[u8],
    stop: &mut dyn FnMut() -> bool,
) -> Result<(), OutletError> {
    while !bytes.is_empty() {
        match conn.write(bytes) {
            Ok(0) => return Err(OutletError::Closed),
            Ok(n) => {
                bytes = &bytes[n..];
                if !bytes.is_empty() && stop() {
                    return Err(OutletError::Stopped);
                }
            }
            Err(e) if timed_out(&e) || e.kind() == ErrorKind::Interrupted => {
                if stop() {
                    return Err(OutletError::Stopped);
                }
            }
            Err(_) => return Err(OutletError::Closed),
        }
    }
    Ok(())
}

/// A stream on the client's connection. The hub sets the write timeout of the connection.
pub struct ConnOutlet {
    conn: Box<dyn Conn>,
    format: SampleType,
    framer: Option<Box<dyn Framer>>,
    head: Vec<u8>,
}

impl ConnOutlet {
    /// The bare samples of `format` on `conn`.
    pub fn new(conn: Box<dyn Conn>, format: SampleType) -> ConnOutlet {
        ConnOutlet {
            conn,
            format,
            framer: None,
            head: Vec::new(),
        }
    }

    /// Packets of samples of `format` on `conn`, each after the header of `framer`.
    pub fn framed(conn: Box<dyn Conn>, format: SampleType, framer: Box<dyn Framer>) -> ConnOutlet {
        ConnOutlet {
            conn,
            format,
            framer: Some(framer),
            head: Vec::new(),
        }
    }
}

impl Outlet for ConnOutlet {
    fn format(&self) -> SampleType {
        self.format
    }

    fn send(
        &mut self,
        samples: &[u8],
        info: &FrameInfo,
        stop: &mut dyn FnMut() -> bool,
    ) -> Result<(), OutletError> {
        let Some(f) = self.framer.as_mut() else {
            return write_patient(self.conn.as_mut(), samples, stop);
        };
        let ss = size(self.format);
        let per = (f.max_payload() / ss).max(1) * ss;
        let mut part = *info;
        for p in samples.chunks(per) {
            self.head.clear();
            f.header(p, &part, &mut self.head);
            write_patient(self.conn.as_mut(), &self.head, stop)?;
            write_patient(self.conn.as_mut(), p, stop)?;
            let n = (p.len() / ss) as u64;
            part.first_sample += n;
            part.time_ns =
                info.time_ns + time_ns(part.first_sample - info.first_sample, part.rate_hz);
            part.gap = Gap::None;
        }
        Ok(())
    }
}

/// The time of sample `index` at `rate_hz`, ns from the start.
pub fn time_ns(index: u64, rate_hz: f64) -> i64 {
    if rate_hz > 0.0 {
        (index as f64 * 1e9 / rate_hz) as i64
    } else {
        0
    }
}
