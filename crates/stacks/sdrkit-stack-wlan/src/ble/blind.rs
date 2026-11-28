//! Connections whose start was not heard (Bluetooth Core Specification v5.4 Vol 6 Part B clauses 2.1.2, 3.1.1, 4.5.1 and
//! 4.5.8). A data channel receiver reports each packet that starts with a preamble and an access address that obeys the
//! rules, with the CRC preset that would make its CRC good. Two packets of one access address with different headers that
//! agree on the preset are a link: its later packets are good or bad by that preset. The headers must differ so that one
//! packet read twice does not look like a link. A packet whose bits are read a few bits early or late can pass the rules
//! too, giving an access address that is the true one shifted; such a reading is ignored once its unshifted link, or a
//! link the receivers already hold, is known, and two shifted relatives wait until one of them wins. The link is listed
//! as a connection once it sends a data PDU
//! (LLID 1 or 2); a link that sends LLID 3 only is a periodic advertising train whose SyncInfo was not heard, and is not
//! listed.
//!
//! The first packet of each connection event gives the event's time and channel. The connection interval is the largest
//! multiple of 1.25 ms that the times of the events are whole multiples of. With the interval, the events are counted, and the channels
//! are tried against channel selection algorithm #1 (one hop increment explains the steps) and algorithm #2 (one event
//! counter explains the channels), both over all 37 data channels: a connection with a reduced channel map fits neither
//! and its algorithm stays unknown.

use super::follow::start_to_aa_end_s;
use sdrkit_proto_wlan_phy::ble::csa::{self, ChannelMap};
use sdrkit_proto_wlan_phy::ble::BlindPacket;
use std::collections::VecDeque;

/// Packets of one access address that must agree on the CRC preset.
pub const CONFIRM_PACKETS: usize = 2;
/// The most access addresses that wait for a second packet.
const CANDIDATES: usize = 512;
/// An access address not heard again for this long is forgotten, s.
const CANDIDATE_LIFE_S: f64 = 30.0;
/// The most links kept.
pub const LINKS: usize = 64;
/// The most events of a link kept for the estimates.
const EVENTS: usize = 64;
/// A packet this long after the start of the link's packet before it starts a new connection event, s.
const EVENT_GAP_S: f64 = 3e-3;
/// 1.25 ms, the unit of the connection interval, s.
const UNIT_S: f64 = 1.25e-3;
/// The events of this long count for the interval, s.
const SPAN_S: f64 = 30.0;
/// The share of the steps an estimate must explain, percent.
const SHARE: usize = 85;

/// The most access addresses of links the receivers hold that are kept, so readings of their packets a few bits off are
/// not taken for a new link.
const HELD: usize = 64;
/// The most bits a reading of a packet may be early or late and still pass the preamble and the rules.
const SHIFTS: u32 = 16;

/// The most bit errors between two readings taken for the same link when one of them is a candidate, not yet a link: the
/// demodulator slices a symbol wrong now and then, as the receiver's own matcher allows for the links it holds.
const NEAR_BITS: u32 = 2;
/// The most bit errors between a new reading and a link already found. It is wider than [`NEAR_BITS`]: a found link has
/// many packets clustered on its true address, so a reading a few more bits off is its own, not a different link (two
/// random 32-bit addresses this close never happen).
const NEAR_LINK_BITS: u32 = 5;

/// Whether `a` and `b` are the same 32-bit string read up to [`SHIFTS`] bits apart: a late reading drops low bits and
/// brings high bits in from the next symbols, an early reading does the reverse, and the overlap must match.
fn shifted(a: u32, b: u32) -> bool {
    (1..=SHIFTS).any(|s| a >> s == b & (u32::MAX >> s) || b >> s == a & (u32::MAX >> s))
}

/// Whether `a` and `b` are readings of the same access address within `bits` sliced wrong, or one string read a few bits
/// apart.
fn near(a: u32, b: u32, bits: u32) -> bool {
    a != b && ((a ^ b).count_ones() <= bits || shifted(a, b))
}

/// Whether `a` and `b` are readings of the same candidate (the tight threshold).
fn related(a: u32, b: u32) -> bool {
    near(a, b, NEAR_BITS)
}

/// An access address heard, with the (preset, header) of its last packets.
#[derive(Clone, Copy, Debug)]
struct Candidate {
    aa: u32,
    last: [(u32, [u8; 2]); 4],
    /// Packets heard.
    n: u32,
    last_s: f64,
}

impl Candidate {
    /// The last packets held (at most four).
    fn held(&self) -> &[(u32, [u8; 2])] {
        &self.last[..(self.n as usize).min(4)]
    }

    /// Whether the last packets all agree on the preset.
    fn clean(&self) -> bool {
        self.held().iter().all(|x| x.0 == self.last[0].0)
    }

    /// Whether the last packets are two or more that agree on the preset and differ in the header: a real link, not one
    /// packet read twice.
    fn agrees(&self) -> bool {
        self.clean() && self.held().iter().any(|x| x.1 != self.last[0].1)
    }
}

/// A link found by two packets that agree on the CRC preset.
#[derive(Clone, Debug)]
pub struct BlindLink {
    /// The access address.
    pub aa: u32,
    /// The CRC preset.
    pub crc_init: u32,
    /// A data PDU was heard: the link is a connection.
    pub connection: bool,
    /// Good packets heard before the link showed to be a connection.
    pub before: u32,
    /// The connection interval, units of 1.25 ms, once estimated.
    pub interval: Option<u32>,
    /// The channel selection algorithm (1 or 2), once one fits.
    pub csa: Option<u8>,
    /// The hop increment of algorithm #1.
    pub hop: Option<u8>,
    /// The (start, channel) of the first packet of the last connection events.
    events: VecDeque<(f64, u8)>,
    count: u32,
    estimated_at: u32,
    last_start_s: f64,
}

/// What a packet was.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// A packet of no listed connection.
    Unknown,
    /// A good packet of a connection; `first` when the connection is listed with it.
    Good {
        /// The connection is new.
        first: bool,
    },
    /// A packet of a connection whose CRC does not fit the connection's preset.
    Bad,
}

/// The largest connection interval (units of 1.25 ms, 6 or more) that the times of the events `e` are whole multiples of;
/// `e` holds (start s, channel) in time order. `None` before five events, or when no interval explains 85 % of them. When
/// every event is on one channel and most steps are one time that 37 divides, the interval is that time over 37:
/// algorithm #1 over all data channels comes back to a channel every 37 events.
pub fn interval_units(e: &[(f64, u8)]) -> Option<u32> {
    let last = e.last()?.0;
    let from = e.partition_point(|x| last - x.0 > SPAN_S);
    let e = &e[from..];
    if e.len() < 5 {
        return None;
    }
    // No interval is longer than a step between two events: the second shortest step bounds it, so one odd step does not.
    let mut steps: Vec<f64> = e.windows(2).map(|w| w[1].0 - w[0].0).collect();
    steps.sort_by(f64::total_cmp);
    let top = (steps[1] / UNIT_S).round() as u32;
    // The whole intervals `n` in time `d`, when `d` is that many intervals long within the clock drift.
    let whole = |d: f64, n: u32| {
        let m = (d / (f64::from(n) * UNIT_S)).round();
        ((d - m * f64::from(n) * UNIT_S).abs() <= 0.2e-3 + 60e-6 * d.abs()).then_some(m as i64)
    };
    let mid = e.len() / 2;
    // The times are taken from an event in the middle; another when that one is an odd one.
    let n = [mid, mid + 1, mid - 1].into_iter().find_map(|r| {
        (6..=top).rev().find(|&n| {
            e.iter().filter(|x| whole(x.0 - e[r].0, n).is_some()).count() * 100 >= SHARE * e.len()
        })
    })?;
    let one_channel = e.iter().all(|x| x.1 == e[0].1);
    let one_step = steps.iter().filter(|&&d| whole(d, n) == Some(1)).count() * 100 >= 70 * steps.len();
    if one_channel && one_step && n.is_multiple_of(37) && n / 37 >= 6 {
        return Some(n / 37);
    }
    Some(n)
}

/// The channel selection algorithm that explains the events `e` of the connection with access address `aa` and interval
/// `interval` (units of 1.25 ms), over all 37 data channels: (1, the hop increment), (2, none), or `None` when the events
/// are too few or fit neither.
pub fn selection(e: &[(f64, u8)], interval: u32, aa: u32) -> Option<(u8, Option<u8>)> {
    let first = e.first()?;
    let step = f64::from(interval) * UNIT_S;
    let k: Vec<i64> = e.iter().map(|x| ((x.0 - first.0) / step).round() as i64).collect();
    // Algorithm #1: the unmapped channel moves by the hop increment each event, modulo 37.
    let pairs: Vec<(i64, u8, u8)> = (1..e.len())
        .map(|i| (k[i] - k[i - 1], e[i - 1].1, e[i].1))
        .filter(|p| p.0 % 37 != 0)
        .collect();
    if pairs.len() >= 6 {
        for hop in 5..=16i64 {
            let fit = pairs
                .iter()
                .filter(|&&(dk, a, b)| (i64::from(a) + hop * dk).rem_euclid(37) == i64::from(b))
                .count();
            if fit * 100 >= SHARE * pairs.len() {
                return Some((1, Some(hop as u8)));
            }
        }
    }
    // Algorithm #2: one event counter of the first event gives every channel.
    if e.len() < 8 {
        return None;
    }
    let (all, id) = (ChannelMap::all(), csa::channel_identifier(aa));
    let allowed = e.len() - (SHARE * e.len()).div_ceil(100);
    let mut found = 0;
    for c0 in 0..=u16::MAX {
        let mut miss = 0;
        for (i, x) in e.iter().enumerate() {
            if csa::csa2_channel(c0.wrapping_add(k[i] as u16), id, &all) != x.1 {
                miss += 1;
                if miss > allowed {
                    break;
                }
            }
        }
        if miss <= allowed {
            found += 1;
        }
    }
    (found == 1).then_some((2, None))
}

impl BlindLink {
    /// Takes a good packet that started at `start_s` on `channel`.
    fn heard(&mut self, start_s: f64, channel: u8) {
        let first = start_s - self.last_start_s > EVENT_GAP_S;
        self.last_start_s = start_s;
        if !first {
            return;
        }
        if self.events.len() == EVENTS {
            self.events.pop_front();
        }
        self.events.push_back((start_s, channel));
        self.count += 1;
        if self.count >= 5 && self.count - self.estimated_at >= 4 && (self.csa.is_none() || self.count < 400) {
            self.estimated_at = self.count;
            let e = self.events.make_contiguous();
            if let Some(n) = interval_units(e) {
                self.interval = Some(n);
                if self.csa.is_none() {
                    if let Some((algorithm, hop)) = selection(e, n, self.aa) {
                        self.csa = Some(algorithm);
                        self.hop = hop;
                    }
                }
            }
        }
    }

    /// The time of the last packet, s.
    pub fn last_s(&self) -> f64 {
        self.last_start_s
    }
}

/// The access addresses heard and the links found (module documentation).
#[derive(Clone, Debug, Default)]
pub struct Blind {
    cands: Vec<Candidate>,
    links: Vec<BlindLink>,
    held: Vec<u32>,
}

impl Blind {
    /// Forgets everything.
    pub fn clear(&mut self) {
        self.cands.clear();
        self.links.clear();
        self.held.clear();
    }

    /// Notes that the receivers hold the link with access address `aa` (advertising, a periodic train, or a connection
    /// followed from its start): readings of its packets a few bits off are then not taken for a new link.
    pub fn hold(&mut self, aa: u32) {
        if self.held.contains(&aa) {
            return;
        }
        if self.held.len() == HELD {
            self.held.remove(0);
        }
        self.held.push(aa);
        self.cands.retain(|c| !near(c.aa, aa, NEAR_LINK_BITS));
    }

    /// The link with access address `aa`.
    pub fn link(&self, aa: u32) -> Option<&BlindLink> {
        self.links.iter().find(|l| l.aa == aa)
    }

    /// The links found.
    pub fn links(&self) -> &[BlindLink] {
        &self.links
    }

    /// Takes packet `p`, whose access address ended at signal time `time_s`.
    pub fn take(&mut self, time_s: f64, p: &BlindPacket) -> Outcome {
        let start = time_s - start_to_aa_end_s(p.phy);
        if let Some(l) = self.links.iter_mut().find(|l| l.aa == p.access_address) {
            if l.crc_init != p.crc_init {
                return if l.connection { Outcome::Bad } else { Outcome::Unknown };
            }
            l.heard(start, p.channel);
            let first = !l.connection && matches!(p.header[0] & 3, 1 | 2);
            l.connection |= first;
            if !l.connection {
                l.before += 1;
                return Outcome::Unknown;
            }
            return Outcome::Good { first };
        }
        let aa = p.access_address;
        // A reading of a held link's packet a few bits off, or of a link already found, is not a new link.
        if self.held.iter().any(|&h| near(h, aa, NEAR_LINK_BITS))
            || self.links.iter().any(|l| near(l.aa, aa, NEAR_LINK_BITS))
        {
            return Outcome::Unknown;
        }
        self.cands.retain(|c| time_s - c.last_s <= CANDIDATE_LIFE_S);
        let Some(i) = self.cands.iter().position(|c| c.aa == aa) else {
            if self.cands.len() == CANDIDATES {
                // The one not heard for the longest time goes.
                if let Some(old) = (0..self.cands.len()).min_by(|&a, &b| self.cands[a].last_s.total_cmp(&self.cands[b].last_s)) {
                    self.cands.swap_remove(old);
                }
            }
            self.cands.push(Candidate {
                aa,
                last: [(p.crc_init, p.header); 4],
                n: 1,
                last_s: time_s,
            });
            return Outcome::Unknown;
        };
        self.cands[i].last_s = time_s;
        self.cands[i].last.rotate_right(1);
        self.cands[i].last[0] = (p.crc_init, p.header);
        self.cands[i].n += 1;
        let c = self.cands[i];
        // A relative read a few bits off that agrees as well, and over as many packets, may be the true link instead:
        // wait until one wins.
        let rival = |r: &Candidate| r.aa != aa && related(r.aa, aa) && r.agrees() && r.n + 1 >= c.n;
        if !c.agrees() || self.cands.iter().any(rival) {
            return Outcome::Unknown;
        }
        // The packets before this one of a preset that did not agree were readings off the mark: they are not counted.
        let before = c.held().iter().filter(|x| x.0 == p.crc_init).count() as u32 - 1;
        self.cands.retain(|r| r.aa != aa && !near(r.aa, aa, NEAR_LINK_BITS));
        if self.links.len() == LINKS {
            if let Some(old) = (0..LINKS).min_by(|&a, &b| self.links[a].last_start_s.total_cmp(&self.links[b].last_start_s)) {
                self.links.swap_remove(old);
            }
        }
        let connection = matches!(p.header[0] & 3, 1 | 2);
        let mut l = BlindLink {
            aa: p.access_address,
            crc_init: p.crc_init,
            connection,
            before,
            interval: None,
            csa: None,
            hop: None,
            events: VecDeque::with_capacity(EVENTS),
            count: 0,
            estimated_at: 0,
            last_start_s: f64::NEG_INFINITY,
        };
        l.heard(start, p.channel);
        self.links.push(l);
        if connection {
            Outcome::Good { first: true }
        } else {
            Outcome::Unknown
        }
    }
}
