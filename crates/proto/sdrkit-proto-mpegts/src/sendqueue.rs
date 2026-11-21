//! The transport stream that a host feeds to a sender of digital television: a bounded queue of 188-byte packets taken in
//! order, one in each packet slot of the modulator. A slot that finds the queue empty carries a null packet (PID 0x1FFF).
//! Each PCR leaves rewritten to the time of its slot (ISO/IEC 13818-1 2.4.2.2; the ±500 ns accuracy clause is not confirmed).
//!
//! Standard: ISO/IEC 13818-1 2.4.3 (packets and adaptation field).

use crate::packet::{parse, NULL_PID, SYNC, TS_SIZE};
use crate::source::{null_packet, PacketSource};
use sdrkit_core::interface::Arguments;
use std::collections::VecDeque;
use std::sync::Arc;

/// The packets the queue holds.
pub const QUEUE_PACKETS: usize = 6000;
/// The 27 MHz clock of the PCR.
const PCR_HZ: f64 = 27e6;
/// The PCR wraps at 2^33 × 300 ticks.
const PCR_WRAP: u64 = (1 << 33) * 300;

/// The arguments of the `append` command of a sender that takes transport packets.
#[derive(Arguments)]
pub struct TxAppendArgs {
    /// The packets.
    #[arg(
        label = "Packets",
        help = "Whole transport packets of 188 bytes, each starting with 0x47. Null packets are left out."
    )]
    pub packets: Arc<[u8]>,
}

/// Why packets were refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QueueError {
    /// The bytes are not a whole number of 188-byte packets.
    NotWhole {
        /// The bytes given.
        bytes: usize,
    },
    /// A packet does not start with the sync byte 0x47.
    NoSync {
        /// The packet, from 0.
        packet: usize,
    },
    /// The queue has no room for the packets: the host must wait.
    Full {
        /// The packets given, null packets left out.
        packets: usize,
        /// The room in the queue.
        room: usize,
    },
}

impl std::fmt::Display for QueueError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            QueueError::NotWhole { bytes } => {
                write!(f, "{bytes} bytes are not whole transport packets of 188 bytes")
            }
            QueueError::NoSync { packet } => {
                write!(f, "packet {packet} does not start with the sync byte 0x47")
            }
            QueueError::Full { packets, room } => write!(
                f,
                "the queue has room for {room} packets, not {packets}: send them later"
            ),
        }
    }
}

impl std::error::Error for QueueError {}

/// The time base of one PCR PID: a PCR value and the slot time it left at.
#[derive(Clone, Copy, Debug)]
struct Anchor {
    pid: u16,
    pcr: u64,
    t: f64,
}

/// The queue of one transport stream and its counts. As a [`PacketSource`] it gives one packet for each slot.
#[derive(Debug)]
pub struct SendQueue {
    packets: VecDeque<[u8; TS_SIZE]>,
    slot_s: f64,
    slot: u64,
    anchors: Vec<Anchor>,
    /// Packets of the host sent.
    pub sent: u64,
    /// Null packets put in slots that found the queue empty.
    pub nulls: u64,
    /// Null packets of the host left out.
    pub dropped_nulls: u64,
    /// PCRs rewritten.
    pub restamped: u64,
}

impl SendQueue {
    /// An empty queue for a stream of `bit_rate` bit/s.
    pub fn new(bit_rate: f64) -> Self {
        let mut f = SendQueue {
            packets: VecDeque::new(),
            slot_s: 0.0,
            slot: 0,
            anchors: Vec::new(),
            sent: 0,
            nulls: 0,
            dropped_nulls: 0,
            restamped: 0,
        };
        f.restart(bit_rate);
        f
    }

    /// Starts the slots again at `bit_rate` bit/s with new counts. The packets that wait stay.
    pub fn restart(&mut self, bit_rate: f64) {
        self.slot_s = if bit_rate > 0.0 {
            (TS_SIZE * 8) as f64 / bit_rate
        } else {
            0.0
        };
        self.slot = 0;
        self.anchors.clear();
        self.sent = 0;
        self.nulls = 0;
        self.dropped_nulls = 0;
        self.restamped = 0;
    }

    /// Adds the packets of `bytes` (whole 188-byte packets), leaving out null packets. Adds all or none.
    ///
    /// # Errors
    ///
    /// [`QueueError`] when the bytes are not whole packets, a packet has no sync byte, or the queue has no room.
    pub fn append(&mut self, bytes: &[u8]) -> Result<usize, QueueError> {
        if !bytes.len().is_multiple_of(TS_SIZE) {
            return Err(QueueError::NotWhole { bytes: bytes.len() });
        }
        let mut nulls = 0;
        for (i, p) in bytes.as_chunks::<TS_SIZE>().0.iter().enumerate() {
            if p[0] != SYNC {
                return Err(QueueError::NoSync { packet: i });
            }
            nulls += usize::from(pid(p) == NULL_PID);
        }
        let n = bytes.len() / TS_SIZE - nulls;
        if n > self.room() {
            return Err(QueueError::Full {
                packets: n,
                room: self.room(),
            });
        }
        for p in bytes.as_chunks::<TS_SIZE>().0 {
            if pid(p) != NULL_PID {
                self.packets.push_back(*p);
            }
        }
        self.dropped_nulls += nulls as u64;
        Ok(n)
    }

    /// The packets that wait.
    pub fn len(&self) -> usize {
        self.packets.len()
    }

    /// Whether no packet waits.
    pub fn is_empty(&self) -> bool {
        self.packets.is_empty()
    }

    /// The room for more packets.
    pub fn room(&self) -> usize {
        QUEUE_PACKETS - self.packets.len()
    }

    /// The time that the waiting packets take at the bit rate, seconds.
    pub fn queued_s(&self) -> f64 {
        self.packets.len() as f64 * self.slot_s
    }

    /// Drops every waiting packet.
    pub fn clear(&mut self) {
        self.packets.clear();
    }

    /// Rewrites the PCR of `p`, which leaves at slot time `t`, to that time on the time base of its PID: the first PCR of
    /// a PID, and a PCR after a discontinuity or more than `jump_s` off its time base, start the time base and stay.
    fn restamp(&mut self, p: &mut [u8; TS_SIZE], t: f64, jump_s: f64) {
        let Some(parsed) = parse(p) else { return };
        let Some(a) = parsed.adaptation else { return };
        let Some(pcr) = a.pcr else { return };
        let pid = parsed.header.pid;
        let at = self.anchors.iter().position(|x| x.pid == pid);
        let fresh = Anchor { pid, pcr, t };
        let Some(i) = at else {
            self.anchors.push(fresh);
            return;
        };
        let base = self.anchors[i];
        let elapsed = (pcr + PCR_WRAP - base.pcr) % PCR_WRAP;
        let due = base.t + elapsed as f64 / PCR_HZ;
        if a.discontinuity || (t - due).abs() > jump_s {
            self.anchors[i] = fresh;
            return;
        }
        let out = (base.pcr + ((t - base.t) * PCR_HZ).round() as u64) % PCR_WRAP;
        if out != pcr {
            write_pcr(p, out);
            self.restamped += 1;
        }
    }
}

impl PacketSource for SendQueue {
    fn next_packet(&mut self, out: &mut [u8; TS_SIZE]) {
        let t = self.slot as f64 * self.slot_s;
        self.slot += 1;
        match self.packets.pop_front() {
            Some(mut p) => {
                let jump_s = QUEUE_PACKETS as f64 * self.slot_s + 1.0;
                self.restamp(&mut p, t, jump_s);
                *out = p;
                self.sent += 1;
            }
            None => {
                *out = null_packet();
                self.nulls += 1;
            }
        }
    }
}

/// The PID of a packet.
fn pid(p: &[u8]) -> u16 {
    (u16::from(p[1] & 0x1F) << 8) | u16::from(p[2])
}

/// Writes `pcr` (27 MHz ticks) into the PCR field of `p`, which has one: base in bytes 6 to 10, extension in 10 and 11, the
/// reserved bits kept.
fn write_pcr(p: &mut [u8; TS_SIZE], pcr: u64) {
    let base = pcr / 300;
    let ext = pcr % 300;
    p[6] = (base >> 25) as u8;
    p[7] = (base >> 17) as u8;
    p[8] = (base >> 9) as u8;
    p[9] = (base >> 1) as u8;
    p[10] = (((base & 1) as u8) << 7) | (p[10] & 0x7E) | ((ext >> 8) as u8 & 1);
    p[11] = ext as u8;
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A packet of `pid` with an adaptation field that holds `pcr`.
    fn pcr_packet(pid: u16, pcr: u64) -> [u8; TS_SIZE] {
        let mut p = [0xFFu8; TS_SIZE];
        p[0] = SYNC;
        p[1] = (pid >> 8) as u8 & 0x1F;
        p[2] = pid as u8;
        p[3] = 0x30;
        p[4] = 7;
        p[5] = 0x10;
        write_pcr(&mut p, pcr);
        p
    }

    #[test]
    fn a_pcr_written_reads_back() {
        for v in [0, 1, 299, 300, 12_345_678_901, PCR_WRAP - 1] {
            let p = pcr_packet(0x100, v);
            assert_eq!(parse(&p).and_then(|x| x.adaptation).and_then(|a| a.pcr), Some(v));
        }
    }

    #[test]
    fn late_pcrs_leave_with_the_time_of_their_slot() {
        // 1 Mbit/s: a slot is 1.504 ms. The stream states 10 ms between PCRs; with its null packets left out they leave in
        // consecutive slots.
        let mut f = SendQueue::new(1e6);
        for k in 0..4u64 {
            f.append(&pcr_packet(0x101, 1_000_000 + k * 270_000)).unwrap();
            f.append(&null_packet()).unwrap();
        }
        let mut out = [0u8; TS_SIZE];
        let mut got = Vec::new();
        for _ in 0..4 {
            f.next_packet(&mut out);
            got.push(parse(&out).unwrap().adaptation.unwrap().pcr.unwrap());
        }
        let slot = 1504e-6 * 27e6;
        for (k, v) in got.iter().enumerate() {
            assert_eq!(*v, 1_000_000 + (k as f64 * slot).round() as u64);
        }
        assert_eq!((f.restamped, f.dropped_nulls, f.sent), (3, 4, 4));
        f.next_packet(&mut out);
        assert_eq!((pid(&out), f.nulls), (NULL_PID, 1));
    }

    #[test]
    fn a_full_queue_refuses_and_keeps_its_packets() {
        let mut f = SendQueue::new(1e6);
        let many: Vec<u8> = (0..QUEUE_PACKETS).flat_map(|_| pcr_packet(0x101, 0)).collect();
        assert_eq!(f.append(&many), Ok(QUEUE_PACKETS));
        assert_eq!(
            f.append(&pcr_packet(0x101, 0)),
            Err(QueueError::Full {
                packets: 1,
                room: 0
            })
        );
        assert_eq!(f.append(&null_packet()), Ok(0));
        assert_eq!(f.append(&[0x47; 100]), Err(QueueError::NotWhole { bytes: 100 }));
        assert_eq!(f.len(), QUEUE_PACKETS);
    }
}
