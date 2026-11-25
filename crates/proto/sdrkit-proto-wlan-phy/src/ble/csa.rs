//! The channel selection algorithms of connections and periodic advertising (Bluetooth Core Specification v5.4 Vol 6 Part B
//! clause 4.5.8): the channel map, algorithm #1 (hop increment) and algorithm #2 (the event pseudo-random number of the access
//! address and the event counter).

/// A channel map of the 37 general-purpose channels (ChM: bit n is channel n, least significant bit first).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ChannelMap {
    used: [u8; 37],
    n: u8,
    bits: u64,
}

impl ChannelMap {
    /// The map of the five ChM octets. Bits above 36 are ignored. `None` when fewer than two channels are used (clause 4.5.8.1:
    /// at least two).
    pub fn from_bytes(chm: [u8; 5]) -> Option<ChannelMap> {
        let bits = chm
            .iter()
            .enumerate()
            .fold(0u64, |a, (i, &b)| a | (b as u64) << (8 * i))
            & ((1u64 << 37) - 1);
        let mut used = [0u8; 37];
        let mut n = 0u8;
        for c in 0..37u8 {
            if bits & (1 << c) != 0 {
                used[n as usize] = c;
                n += 1;
            }
        }
        (n >= 2).then_some(ChannelMap { used, n, bits })
    }

    /// Every channel used.
    pub fn all() -> ChannelMap {
        ChannelMap::from_bytes([0xFF, 0xFF, 0xFF, 0xFF, 0x1F]).unwrap_or(ChannelMap {
            used: [0; 37],
            n: 0,
            bits: 0,
        })
    }

    /// The used channels in ascending order (the remapping table).
    pub fn used(&self) -> &[u8] {
        &self.used[..self.n as usize]
    }

    /// Whether channel `c` is used.
    pub fn is_used(&self, c: u8) -> bool {
        c < 37 && self.bits & (1 << c) != 0
    }

    /// The five ChM octets.
    pub fn bytes(&self) -> [u8; 5] {
        let b = self.bits.to_le_bytes();
        [b[0], b[1], b[2], b[3], b[4]]
    }
}

/// Channel selection algorithm #1 (clause 4.5.8.2): `unmappedChannel = (lastUnmappedChannel + hopIncrement) mod 37`, remapped
/// by `unmappedChannel mod numUsedChannels` when unused. `lastUnmappedChannel` is 0 before the first connection event.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Csa1 {
    hop: u8,
    last: u8,
}

impl Csa1 {
    /// The algorithm with hop increment `hop` (5 to 16 in a CONNECT_IND), before the first connection event.
    pub fn new(hop: u8) -> Csa1 {
        Csa1 {
            hop: hop % 37,
            last: 0,
        }
    }

    /// The data channel of the next connection event.
    pub fn next(&mut self, map: &ChannelMap) -> u8 {
        let unmapped = (self.last + self.hop) % 37;
        self.last = unmapped;
        if map.is_used(unmapped) || map.used().is_empty() {
            unmapped
        } else {
            map.used()[unmapped as usize % map.used().len()]
        }
    }

    /// The data channel of connection event `counter` (0 the first), from the start.
    pub fn channel_of(hop: u8, counter: u16, map: &ChannelMap) -> u8 {
        let unmapped = ((counter as u32 + 1) * (hop % 37) as u32 % 37) as u8;
        if map.is_used(unmapped) || map.used().is_empty() {
            unmapped
        } else {
            map.used()[unmapped as usize % map.used().len()]
        }
    }
}

/// The channel identifier of an access address (clause 4.5.8.3.2): its upper 16 bits XOR its lower 16 bits.
pub fn channel_identifier(access_address: u32) -> u16 {
    (access_address >> 16) as u16 ^ access_address as u16
}

/// The permutation of clause 4.5.8.3.2: each octet bit-reversed in place.
fn perm(x: u16) -> u16 {
    ((x >> 8) as u8).reverse_bits() as u16 * 256 + (x as u8).reverse_bits() as u16
}

/// The Multiply, Add and Modulo block: `(17 a + b) mod 2^16`.
fn mam(a: u16, b: u16) -> u16 {
    (17u32 * a as u32 + b as u32) as u16
}

/// prn_e of channel selection algorithm #2 (figure 4.53): the counter XOR the channel identifier, three rounds of PERM and MAM,
/// XOR the channel identifier.
pub fn prn_e(counter: u16, channel_id: u16) -> u16 {
    let mut x = counter ^ channel_id;
    for _ in 0..3 {
        x = mam(perm(x), channel_id);
    }
    x ^ channel_id
}

/// The channel of an event by channel selection algorithm #2 (clauses 4.5.8.3.3 and 4.5.8.3.4): `prn_e mod 37` when used, else
/// the used channel at `floor(N prn_e / 2^16)`.
pub fn csa2_channel(counter: u16, channel_id: u16, map: &ChannelMap) -> u8 {
    let p = prn_e(counter, channel_id);
    let unmapped = (p % 37) as u8;
    let used = map.used();
    if map.is_used(unmapped) || used.is_empty() {
        unmapped
    } else {
        used[((used.len() as u32 * p as u32) >> 16) as usize]
    }
}
