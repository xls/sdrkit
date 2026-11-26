//! The streams of SoapyRemote, over UDP datagrams or TCP messages ([`Wire`]). Receive: messages of a 24-byte header
//! (length, sequence, elements, flags, time) and the samples, sent while the sequence is inside the window that the
//! client's last acknowledgement gave; a gap of the source or of the client's own queue goes as a message with the overflow
//! code. Transmit: the client's messages go into the core's transmit ring in order and are acknowledged within the window;
//! an underrun of the owner goes back as an underflow on the status path.

use sdrkit_driver_soapyremote::stream::{
    ack, ack_trigger, seq_event, DgHeader, SeqEvent, END_BURST, HAS_TIME, HEADER_LEN, OVERFLOW,
    UNDERFLOW,
};
use sdrkit_driver_soapyremote::wire::Wire;
use sdrkit_serve_core::{time_ns, FrameInfo, Gap, Outlet, OutletError, SampleType, TxWriter};
use std::io::ErrorKind;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

/// IPv4 and UDP headers, bytes.
const IP_UDP: usize = 28;
/// How long a sender waits for an acknowledgement before it checks for a stop.
const ACK_POLL: Duration = Duration::from_millis(5);

/// The elements of `bytes` bytes each that fit one message of `mtu` bytes.
pub(crate) fn elements_per_datagram(mtu: usize, bytes: usize) -> usize {
    (mtu.saturating_sub(IP_UDP + HEADER_LEN) / bytes.max(1)).max(1)
}

/// The sending end of a receive stream. It lives from the set-up to the close of the stream, so the sequence and the
/// window carry on over a deactivate and an activate.
pub(crate) struct RxLink {
    data: Wire,
    bpe: usize,
    per: usize,
    seq: u32,
    acked: u32,
    window: u32,
    packet: Vec<u8>,
    ackbuf: [u8; 64],
}

impl RxLink {
    /// Sends samples of `bpe` bytes each on `data`, at most `mtu` bytes a message.
    pub(crate) fn new(mut data: Wire, bpe: usize, mtu: usize) -> RxLink {
        let _ = data.set_read_timeout(Some(ACK_POLL));
        let per = elements_per_datagram(mtu, bpe);
        RxLink {
            data,
            bpe,
            per,
            seq: 0,
            acked: 0,
            window: 0,
            packet: Vec::with_capacity(HEADER_LEN + per * bpe),
            ackbuf: [0; 64],
        }
    }

    fn take_ack(&mut self, n: usize) {
        if let Some(h) = DgHeader::parse(&self.ackbuf[..n]) {
            self.acked = h.seq;
            self.window = h.elems.max(1) as u32;
        }
    }

    fn open(&self) -> bool {
        self.window > 0 && self.seq.wrapping_sub(self.acked) < self.window
    }

    /// Waits until the next sequence is inside the window. UDP takes the acknowledgements that wait first; TCP reads them
    /// only while the window is shut, since each read there waits.
    fn room(&mut self, halt: &mut dyn FnMut() -> bool) -> Result<(), OutletError> {
        loop {
            if !self.data.is_tcp() {
                while let Ok(Some(n)) = self.data.try_recv(&mut self.ackbuf) {
                    self.take_ack(n);
                }
            }
            if self.open() {
                return Ok(());
            }
            if halt() {
                return Err(OutletError::Stopped);
            }
            match self.data.recv(&mut self.ackbuf) {
                Ok(Some(n)) => self.take_ack(n),
                Ok(None) => {}
                Err(_) => return Err(OutletError::Closed),
            }
        }
    }

    /// Sends one message. A TCP message is sent whole unless `stop` ends the client.
    fn send_one(
        &mut self,
        elems: i32,
        payload: &[u8],
        time: i64,
        stop: &mut dyn FnMut() -> bool,
    ) -> Result<(), OutletError> {
        let h = DgHeader {
            bytes: (HEADER_LEN + payload.len()) as u32,
            seq: self.seq,
            elems,
            flags: HAS_TIME,
            time,
        };
        self.packet.clear();
        self.packet.extend_from_slice(&h.to_bytes());
        self.packet.extend_from_slice(payload);
        match self.data.send(&self.packet, stop) {
            Ok(()) => {}
            Err(e) if e.kind() == ErrorKind::TimedOut => return Err(OutletError::Stopped),
            Err(_) => return Err(OutletError::Closed),
        }
        self.seq = self.seq.wrapping_add(1);
        Ok(())
    }
}

fn lock(l: &Mutex<RxLink>) -> MutexGuard<'_, RxLink> {
    l.lock().unwrap_or_else(PoisonError::into_inner)
}

/// One activation of a receive stream. Once `off` is set (deactivate, close) it discards what it is given and stops waiting
/// for the window, so the hub's writer moves on to the next outlet; a message already started is still sent whole, and its
/// failure then is not the client's end.
pub(crate) struct StreamOutlet {
    link: Arc<Mutex<RxLink>>,
    format: SampleType,
    off: Arc<AtomicBool>,
}

impl StreamOutlet {
    /// An outlet of samples of `format` on `link`, until `off`.
    pub(crate) fn new(
        link: Arc<Mutex<RxLink>>,
        format: SampleType,
        off: Arc<AtomicBool>,
    ) -> StreamOutlet {
        StreamOutlet { link, format, off }
    }
}

impl Outlet for StreamOutlet {
    fn format(&self) -> SampleType {
        self.format
    }

    fn path(&self) -> &'static str {
        if lock(&self.link).data.is_tcp() {
            "tcp"
        } else {
            "udp"
        }
    }

    fn send(
        &mut self,
        samples: &[u8],
        info: &FrameInfo,
        stop: &mut dyn FnMut() -> bool,
    ) -> Result<(), OutletError> {
        let off = &*self.off;
        let mut l = lock(&self.link);
        let room = |l: &mut RxLink, stop: &mut dyn FnMut() -> bool| {
            if off.load(Ordering::Acquire) {
                return Ok(false);
            }
            match l.room(&mut || off.load(Ordering::Acquire) || stop()) {
                Ok(()) => Ok(true),
                Err(OutletError::Stopped) if off.load(Ordering::Acquire) => Ok(false),
                Err(e) => Err(e),
            }
        };
        if matches!(info.gap, Gap::Lost(_)) {
            if !room(&mut l, stop)? {
                return Ok(());
            }
            sent(l.send_one(OVERFLOW, &[], info.time_ns, stop), off)?;
        }
        let mut index = info.first_sample;
        let (per, bpe) = (l.per, l.bpe);
        for part in samples.chunks(per * bpe) {
            if !room(&mut l, stop)? {
                return Ok(());
            }
            let n = part.len() / bpe;
            sent(
                l.send_one(
                    n as i32,
                    part,
                    info.time_ns + time_ns(index - info.first_sample, info.rate_hz),
                    stop,
                ),
                off,
            )?;
            index += n as u64;
        }
        Ok(())
    }
}

/// The result of a send: a failure after `off` was set ends only the outlet.
fn sent(r: Result<(), OutletError>, off: &AtomicBool) -> Result<(), OutletError> {
    match r {
        Err(OutletError::Closed) if off.load(Ordering::Acquire) => Ok(()),
        r => r,
    }
}

/// The paths of a transmit stream.
pub(crate) struct TxEnds {
    pub data: Wire,
    pub status: Wire,
}

/// Receives the client's transmit messages into `w` until `stop`, then takes the messages that wait already, so that a
/// client that stops its stream right after its last samples loses none. Messages of `bpe`-byte samples; the window is
/// `window` messages.
pub(crate) fn tx_loop(
    mut e: TxEnds,
    mut w: TxWriter,
    bpe: usize,
    window: u32,
    stop: &Arc<AtomicBool>,
) {
    let stopped = || stop.load(Ordering::Acquire);
    let _ = e.data.set_read_timeout(Some(ACK_POLL * 4));
    let _ = e.data.send(&ack(0, window), &mut || stopped());
    let (mut next, mut acked, mut reported) = (0u32, 0u32, 0u64);
    let mut buf = vec![0u8; 65_536];
    loop {
        let stopping = stopped();
        let under = w.counts().underrun_events;
        if under > reported {
            reported = under;
            let h = DgHeader {
                bytes: HEADER_LEN as u32,
                seq: 1,
                elems: UNDERFLOW,
                flags: 0,
                time: 0,
            };
            let _ = e.status.send(&h.to_bytes(), &mut || stopped());
        }
        let received = if stopping {
            // Only what waits already: nothing more is coming.
            e.data.try_recv(&mut buf)
        } else {
            e.data.recv(&mut buf)
        };
        let n = match received {
            Ok(Some(n)) => n,
            Ok(None) if !stopping => continue,
            Ok(None) | Err(_) => return,
        };
        let Some(h) = DgHeader::parse(&buf[..n]) else {
            continue;
        };
        if seq_event(next, h.seq) == SeqEvent::Late {
            continue;
        }
        next = h.seq.wrapping_add(1);
        let len = (h.elems.max(0) as usize * bpe).min(n.saturating_sub(HEADER_LEN));
        let mut payload = &buf[HEADER_LEN..HEADER_LEN + len];
        while !payload.is_empty() {
            let k = w.write(payload);
            payload = &payload[k..];
            if k == 0 {
                if stopping || stopped() || w.closed() {
                    return;
                }
                std::thread::sleep(Duration::from_millis(1));
            }
        }
        if h.flags & END_BURST != 0 {
            w.end_burst();
        }
        if next.wrapping_sub(acked) >= ack_trigger(window) {
            acked = next;
            let _ = e.data.send(&ack(next, window), &mut || stopped());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn datagrams_fit_the_mtu() {
        assert_eq!(elements_per_datagram(1500, 4), 362);
        assert_eq!(elements_per_datagram(9000, 8), 1118);
        assert_eq!(elements_per_datagram(10, 8), 1);
    }
}
