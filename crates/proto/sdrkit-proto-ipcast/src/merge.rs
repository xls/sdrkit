//! The transport stream of a data PLP with the common PLP of its group put back. Both partial streams keep the packet slots
//! of the stream they were split from, so after null packet reinsertion a slot takes the packet of the common PLP when the
//! data PLP has a null packet there, and the packet of the data PLP otherwise.
//!
//! The two streams line up by packet count. A slot that holds a packet in both streams shows that they do not line up (a
//! deframer started on another slot, or lost packets): [`CommonMerge`] then holds both streams until exactly one offset puts
//! no two packets in a slot over at least [`ALIGN_PACKETS`] packets of the common PLP, and goes on from that offset.
//!
//! Standard: ETSI EN 302 755 V1.4.1 annex D.

use std::collections::VecDeque;

/// The most transport packets one stream gets ahead of the other before the merge passes the data PLP through alone.
pub const MERGE_SLACK: usize = 20_000;

/// The packets of the common PLP that one offset must place in null slots of the data PLP before the merge takes it.
pub const ALIGN_PACKETS: usize = 16;

/// The largest offset between the streams, in packets, that the merge searches.
pub const ALIGN_SPAN: usize = 1024;

/// Whether a transport packet is a null packet (PID 0x1FFF).
pub fn is_null(p: &[u8]) -> bool {
    p.len() >= 3 && ((u16::from(p[1]) & 0x1F) << 8 | u16::from(p[2])) == 0x1FFF
}

#[derive(Clone, Copy)]
struct Slot {
    p: [u8; 188],
    ok: bool,
    null: bool,
}

/// Counters of the merge.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MergeStats {
    /// Packets of the common PLP put into the data PLP.
    pub common: u64,
    /// Slots that held a packet in both streams.
    pub collisions: u64,
    /// Offsets found after streams that did not line up.
    pub realigned: u64,
}

/// The merge of one data PLP with its common PLP. Packets go in with [`CommonMerge::push_data`] and
/// [`CommonMerge::push_common`] and come out with [`CommonMerge::drain`]. It does not allocate once its queues have grown.
pub struct CommonMerge {
    data: VecDeque<Slot>,
    common: VecDeque<Slot>,
    with_common: bool,
    aligned: bool,
    /// The packets of the common PLP queued at which the next search for an offset runs.
    next_try: usize,
    /// The counters.
    pub stats: MergeStats,
}

impl Default for CommonMerge {
    fn default() -> Self {
        CommonMerge {
            data: VecDeque::new(),
            common: VecDeque::new(),
            with_common: false,
            aligned: true,
            next_try: 0,
            stats: MergeStats::default(),
        }
    }
}

impl CommonMerge {
    /// Whether a common PLP takes part. A change forgets the queued packets.
    pub fn set_common(&mut self, on: bool) {
        if on != self.with_common {
            self.with_common = on;
            self.reset();
        }
    }

    /// Whether a common PLP takes part.
    pub fn with_common(&self) -> bool {
        self.with_common
    }

    /// Forgets the queued packets; the next packets of both streams line up by count.
    pub fn reset(&mut self) {
        self.data.clear();
        self.common.clear();
        self.aligned = true;
        self.next_try = 0;
    }

    /// A packet of the data PLP (`ok` false when its CRC-8 failed). A packet that is not 188 bytes long is ignored.
    pub fn push_data(&mut self, p: &[u8], ok: bool) {
        if let Ok(a) = <[u8; 188]>::try_from(p) {
            self.data.push_back(Slot { p: a, ok, null: is_null(p) });
        }
    }

    /// A packet of the common PLP. A packet that is not 188 bytes long is ignored.
    pub fn push_common(&mut self, p: &[u8], ok: bool) {
        if let Ok(a) = <[u8; 188]>::try_from(p) {
            self.common.push_back(Slot { p: a, ok, null: is_null(p) });
        }
    }

    /// Gives the merged packets that are ready to `out(packet, ok)`. Without a common PLP every data packet is ready.
    pub fn drain(&mut self, mut out: impl FnMut(&[u8; 188], bool)) {
        if !self.with_common {
            while let Some(d) = self.data.pop_front() {
                out(&d.p, d.ok);
            }
            return;
        }
        loop {
            if !self.aligned && !self.align(&mut out) {
                break;
            }
            while let (Some(d), Some(c)) = (self.data.front(), self.common.front()) {
                if !d.null && !c.null {
                    self.stats.collisions += 1;
                    self.aligned = false;
                    self.next_try = 0;
                    break;
                }
                let (d, c) = (*d, *c);
                self.data.pop_front();
                self.common.pop_front();
                if d.null && !c.null {
                    self.stats.common += 1;
                    out(&c.p, c.ok);
                } else {
                    out(&d.p, d.ok);
                }
            }
            if self.aligned || self.data.is_empty() || self.common.is_empty() {
                break;
            }
        }
        if self.data.len() > MERGE_SLACK || self.common.len() > MERGE_SLACK {
            // The common PLP has stopped or never lines up: pass the data PLP through and start again.
            while let Some(d) = self.data.pop_front() {
                out(&d.p, d.ok);
            }
            self.common.clear();
            self.aligned = true;
            self.next_try = 0;
        }
    }

    /// Searches the one offset of the streams, at most [`ALIGN_SPAN`] packets each way, that puts no two packets in a slot over
    /// at least [`ALIGN_PACKETS`] packets of the common PLP. It fails while another offset is not ruled out by two packets in
    /// one slot. On success the packets before the line-up are dropped (common PLP) or given out alone (data PLP), and it
    /// returns true.
    fn align(&mut self, out: &mut impl FnMut(&[u8; 188], bool)) -> bool {
        let queued = self.common.iter().filter(|c| !c.null).count();
        if queued < ALIGN_PACKETS.max(self.next_try) {
            return false;
        }
        self.next_try = queued + ALIGN_PACKETS;
        let (nd, nc) = (self.data.len() as isize, self.common.len() as isize);
        let span = ALIGN_SPAN as isize;
        // Offset `o`: data slot `i` lines up with common slot `i + o`. Some(true): it fits; Some(false): two packets share a
        // slot; None: too few packets of the common PLP to tell.
        let fits = |o: isize| {
            let mut placed = 0;
            for i in 0.max(-o)..nd.min(nc - o) {
                let (d, c) = (&self.data[i as usize], &self.common[(i + o) as usize]);
                if !c.null {
                    if !d.null {
                        return Some(false);
                    }
                    placed += 1;
                }
            }
            (placed >= ALIGN_PACKETS).then_some(true)
        };
        // The offset must be the only one that fits while every other one is ruled out: a pattern that repeats (tables
        // sent at a fixed period) fits at several.
        let mut found = None;
        for o in -span..=span {
            match fits(o) {
                Some(false) => {}
                Some(true) if found.is_none() => found = Some(o),
                _ => return false,
            }
        }
        let Some(o) = found else { return false };
        if o > 0 {
            self.common.drain(..o as usize);
        } else {
            for d in self.data.drain(..(-o) as usize) {
                out(&d.p, d.ok);
            }
        }
        self.aligned = true;
        self.stats.realigned += 1;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn packet(pid: u16, n: u32) -> [u8; 188] {
        let mut p = [0u8; 188];
        p[0] = 0x47;
        p[1] = (pid >> 8) as u8;
        p[2] = pid as u8;
        p[4..8].copy_from_slice(&n.to_be_bytes());
        p
    }

    type Packets = Vec<[u8; 188]>;

    /// A stream of `n` packets split into a data and a common stream: about one packet in four goes to the common stream.
    fn split(n: u32) -> (Packets, Packets, Packets) {
        let null = packet(0x1FFF, 0);
        let (mut all, mut d, mut c) = (Vec::new(), Vec::new(), Vec::new());
        let mut x = 0x2545_F491_4F6C_DD1Du64;
        for k in 0..n {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            let common = x >> 62 == 3;
            let p = packet(if common { 0x11 } else { 0x100 }, k);
            all.push(p);
            if common {
                d.push(null);
                c.push(p);
            } else {
                d.push(p);
                c.push(null);
            }
        }
        (all, d, c)
    }

    fn run(m: &mut CommonMerge, d: &[[u8; 188]], c: &[[u8; 188]]) -> Vec<[u8; 188]> {
        let mut got = Vec::new();
        for k in (0..d.len().max(c.len())).step_by(40) {
            for p in d.iter().skip(k).take(40) {
                m.push_data(p, true);
            }
            for p in c.iter().skip(k).take(40) {
                m.push_common(p, true);
            }
            m.drain(|p, _| got.push(*p));
        }
        got
    }

    #[test]
    fn streams_that_start_together_merge_by_count() {
        let (all, d, c) = split(2000);
        let mut m = CommonMerge::default();
        m.set_common(true);
        assert_eq!(run(&mut m, &d, &c), all);
        assert_eq!(m.stats.realigned, 0);
    }

    #[test]
    fn streams_that_start_apart_line_up() {
        let (all, d, c) = split(4000);
        for (skip_d, skip_c) in [(0usize, 37usize), (91, 0), (500, 3)] {
            let mut m = CommonMerge::default();
            m.set_common(true);
            let got = run(&mut m, &d[skip_d..], &c[skip_c..]);
            // Packets merged before the streams were seen not to line up may sit in wrong slots; from the line-up on the
            // stream is whole.
            assert_eq!(&got[got.len() - 3000..], &all[1000..], "skip {skip_d} {skip_c}");
            assert!(m.stats.realigned <= 1, "skip {skip_d} {skip_c}: {:?}", m.stats);
        }
    }

    #[test]
    fn lost_packets_line_up_again() {
        let (all, d, mut c) = split(4000);
        c.drain(1000..1013);
        let mut m = CommonMerge::default();
        m.set_common(true);
        let got = run(&mut m, &d, &c);
        assert_eq!(&got[..1000], &all[..1000]);
        let tail = &all[all.len() - 1500..];
        assert_eq!(&got[got.len() - 1500..], tail);
        assert_eq!(m.stats.realigned, 1);
    }

    #[test]
    fn a_repeating_pattern_is_never_guessed() {
        // Every eighth slot goes to the common stream: every eighth offset fits, so the merge must not pick one.
        let null = packet(0x1FFF, 0);
        let n = 30_000u32;
        let (mut d, mut c) = (Vec::new(), Vec::new());
        for k in 0..n {
            let p = packet(if k % 8 == 0 { 0x11 } else { 0x100 }, k);
            d.push(if k % 8 == 0 { null } else { p });
            c.push(if k % 8 == 0 { p } else { null });
        }
        let mut m = CommonMerge::default();
        m.set_common(true);
        let got = run(&mut m, &d, &c[3..]);
        assert_eq!(m.stats.realigned, 0);
        assert!(got.len() > MERGE_SLACK);
        let numbers: Vec<u32> = got
            .iter()
            .filter(|p| !is_null(&p[..]))
            .map(|p| u32::from_be_bytes([p[4], p[5], p[6], p[7]]))
            .collect();
        assert!(numbers.windows(2).all(|w| w[1] > w[0]));
    }
}
