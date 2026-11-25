//! The UDP streams of SoapyRemote. Receive: datagrams of a 24-byte header (length, sequence, elements, flags, time) and the
//! samples, sent while the sequence is inside the window that the client's last acknowledgement gave; a gap of the source
//! or of the client's own queue goes as a datagram with the overflow code. Transmit: the client's datagrams go into the
//! core's transmit ring in order and are acknowledged within the window; an underrun of the owner goes back as an underflow
//! on the status socket.

use sdrkit_core::net::{timed_out, Datagram};
use sdrkit_driver_soapyremote::stream::{
    ack, ack_trigger, seq_event, DgHeader, SeqEvent, END_BURST, HAS_TIME, HEADER_LEN, OVERFLOW,
    UNDERFLOW,
};
use sdrkit_serve_core::{time_ns, FrameInfo, Gap, Outlet, OutletError, SampleType, TxWriter};
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

/// IPv4 and UDP headers, bytes.
const IP_UDP: usize = 28;
/// How long a sender waits for an acknowledgement before it checks for a stop.
const ACK_POLL: Duration = Duration::from_millis(5);

/// The elements of `bytes` bytes each that fit one datagram of `mtu` bytes.
pub(crate) fn elements_per_datagram(mtu: usize, bytes: usize) -> usize {
    (mtu.saturating_sub(IP_UDP + HEADER_LEN) / bytes.max(1)).max(1)
}

/// The receive stream to one client.
pub(crate) struct UdpOutlet {
    data: Box<dyn Datagram>,
    to: SocketAddr,
    format: SampleType,
    bpe: usize,
    per: usize,
    seq: u32,
    acked: u32,
    window: u32,
    packet: Vec<u8>,
    ackbuf: [u8; 64],
}

impl UdpOutlet {
    /// Sends samples of `format` (`bpe` bytes each) from `data` to `to`, at most `mtu` bytes a datagram.
    pub(crate) fn new(
        mut data: Box<dyn Datagram>,
        to: SocketAddr,
        format: SampleType,
        bpe: usize,
        mtu: usize,
    ) -> UdpOutlet {
        let _ = data.set_read_timeout(Some(ACK_POLL));
        UdpOutlet {
            data,
            to,
            format,
            bpe,
            per: elements_per_datagram(mtu, bpe),
            seq: 0,
            acked: 0,
            window: 0,
            packet: Vec::new(),
            ackbuf: [0; 64],
        }
    }

    fn take_ack(&mut self, n: usize, from: SocketAddr) {
        if from.ip() != self.to.ip() {
            return;
        }
        if let Some(h) = DgHeader::parse(&self.ackbuf[..n]) {
            self.acked = h.seq;
            self.window = h.elems.max(1) as u32;
        }
    }

    /// Waits until the next sequence is inside the window.
    fn room(&mut self, stop: &mut dyn FnMut() -> bool) -> Result<(), OutletError> {
        loop {
            while let Ok(Some((n, from))) = self.data.try_recv_from(&mut self.ackbuf) {
                self.take_ack(n, from);
            }
            if self.window > 0 && self.seq.wrapping_sub(self.acked) < self.window {
                return Ok(());
            }
            if stop() {
                return Err(OutletError::Stopped);
            }
            match self.data.recv_from(&mut self.ackbuf) {
                Ok((n, from)) => self.take_ack(n, from),
                Err(e) if timed_out(&e) || e.kind() == std::io::ErrorKind::Interrupted => {}
                Err(_) => return Err(OutletError::Closed),
            }
        }
    }

    fn send_one(&mut self, elems: i32, payload: &[u8], time: i64) -> Result<(), OutletError> {
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
        self.data
            .send_to(&self.packet, self.to)
            .map_err(|_| OutletError::Closed)?;
        self.seq = self.seq.wrapping_add(1);
        Ok(())
    }
}

impl Outlet for UdpOutlet {
    fn format(&self) -> SampleType {
        self.format
    }

    fn send(
        &mut self,
        samples: &[u8],
        info: &FrameInfo,
        stop: &mut dyn FnMut() -> bool,
    ) -> Result<(), OutletError> {
        if matches!(info.gap, Gap::Lost(_)) {
            self.room(stop)?;
            self.send_one(OVERFLOW, &[], info.time_ns)?;
        }
        let mut index = info.first_sample;
        for part in samples.chunks(self.per * self.bpe) {
            self.room(stop)?;
            let n = part.len() / self.bpe;
            self.send_one(n as i32, part, time_ns(index, info.rate_hz))?;
            index += n as u64;
        }
        Ok(())
    }
}

/// The sockets and addresses of a transmit stream.
pub(crate) struct TxEnds {
    pub data: Box<dyn Datagram>,
    pub status: Box<dyn Datagram>,
    pub data_to: SocketAddr,
    pub status_to: SocketAddr,
}

/// Receives the client's transmit datagrams into `w` until `stop`. Datagrams of `bpe`-byte samples; the window is `window`
/// datagrams.
pub(crate) fn tx_loop(
    mut e: TxEnds,
    mut w: TxWriter,
    bpe: usize,
    window: u32,
    stop: &Arc<AtomicBool>,
) {
    let _ = e.data.set_read_timeout(Some(ACK_POLL * 4));
    let _ = e.data.send_to(&ack(0, window), e.data_to);
    let (mut next, mut acked, mut reported) = (0u32, 0u32, 0u64);
    let mut buf = vec![0u8; 65_536];
    while !stop.load(Ordering::Acquire) {
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
            let _ = e.status.send_to(&h.to_bytes(), e.status_to);
        }
        let (n, from) = match e.data.recv_from(&mut buf) {
            Ok(x) => x,
            Err(err) if timed_out(&err) || err.kind() == std::io::ErrorKind::Interrupted => {
                continue
            }
            Err(_) => return,
        };
        if from.ip() != e.data_to.ip() {
            continue;
        }
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
                if stop.load(Ordering::Acquire) || w.closed() {
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
            let _ = e.data.send_to(&ack(next, window), e.data_to);
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
