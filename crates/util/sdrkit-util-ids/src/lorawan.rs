//! LoRaWAN network identities: the NetID types and the DevAddr layout of LoRaWAN Backend Interfaces TS002 (clause "DevAddr
//! Assignment", NwkID lengths as corrected by LoRa Alliance change request TC22-00003, 22 May 2017), and the operator each NetID
//! is allocated to.
//!
//! **Data.** `data/lorawan_netid.tsv` is the LoRa Alliance "NetID Allocations" list R119 (July 2026,
//! <https://lora-alliance.org/wp-content/uploads/2026/07/NetID-Allocations_R119.pdf>), unassigned NetIDs left out. Type 7 NetIDs
//! are allocated in runs of 16.
//!
//! | NetID type | DevAddr prefix | NwkID bits | NwkAddr bits | NetID ID bits |
//! |---|---|---|---|---|
//! | 0 | `0` | 6 | 25 | 6 |
//! | 1 | `10` | 6 | 24 | 6 |
//! | 2 | `110` | 9 | 20 | 9 |
//! | 3 | `1110` | 11 | 17 | 21 |
//! | 4 | `11110` | 12 | 15 | 21 |
//! | 5 | `111110` | 13 | 13 | 21 |
//! | 6 | `1111110` | 15 | 10 | 21 |
//! | 7 | `11111110` | 17 | 7 | 21 |
//!
//! For types 0 to 2 the NwkID is the whole NetID ID, so a DevAddr names one NetID. For types 3 to 7 the NwkID is only the low
//! bits of the 21-bit ID; [`DevAddr::net_id`] returns the NetID whose high ID bits are zero, which is the allocated one for every
//! allocation in R119.
//!
//! ```
//! use sdrkit_util_ids::lorawan::{DevAddr, NetId};
//!
//! let d = DevAddr(0x2601_1234);
//! assert_eq!(d.net_id().map(|n| n.0), Some(0x000013));
//! assert_eq!(d.operator(), Some("The Things Network"));
//! assert_eq!(NetId(0x60_0001).operator(), Some("Digita"));
//! ```

use crate::pool;

/// NwkID bits of each NetID type.
const NWKID_BITS: [u32; 8] = [6, 6, 9, 11, 12, 13, 15, 17];

/// A 24-bit NetID.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NetId(pub u32);

impl NetId {
    /// Whether the value is a NetID: 24 bits, and for types 0 to 2 the RFU bits between the type and the ID are zero.
    pub const fn is_valid(self) -> bool {
        if self.0 > 0xFF_FFFF {
            return false;
        }
        let id_bits = match self.0 >> 21 {
            0 | 1 => 6,
            2 => 9,
            _ => 21,
        };
        (self.0 & 0x1F_FFFF) >> id_bits == 0
    }

    /// The type (the three high bits).
    pub const fn net_type(self) -> u8 {
        ((self.0 >> 21) & 7) as u8
    }

    /// The ID (the low 21 bits; for types 0 to 2 only the low 6 or 9 bits can be set).
    pub const fn id(self) -> u32 {
        self.0 & 0x1F_FFFF
    }

    /// The operator the NetID is allocated to (`None` when unassigned or invalid). NetIDs 0 and 1 give
    /// "Private/experimental nodes".
    pub fn operator(self) -> Option<&'static str> {
        if !self.is_valid() {
            return None;
        }
        let i = pool::NETID_FIRST
            .partition_point(|&f| f <= self.0)
            .checked_sub(1)?;
        let first = pool::NETID_FIRST[i];
        (self.0 < first + pool::NETID_COUNT[i] as u32)
            .then(|| pool::NETID_POOL.get(pool::NETID_NAME[i]))
    }
}

/// A 32-bit device address.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DevAddr(pub u32);

impl DevAddr {
    /// The NetID type the address prefix names (`None` for a prefix of eight ones, which is reserved).
    pub const fn net_type(self) -> Option<u8> {
        let t = self.0.leading_ones();
        if t >= 8 {
            None
        } else {
            Some(t as u8)
        }
    }

    /// The NwkID field.
    pub const fn nwk_id(self) -> Option<u32> {
        match self.net_type() {
            Some(t) => {
                let bits = NWKID_BITS[t as usize];
                let shift = 32 - (t as u32 + 1) - bits;
                Some((self.0 >> shift) & ((1 << bits) - 1))
            }
            None => None,
        }
    }

    /// The NwkAddr field (the address of the device within its network).
    pub const fn nwk_addr(self) -> Option<u32> {
        match self.net_type() {
            Some(t) => {
                let bits = 32 - (t as u32 + 1) - NWKID_BITS[t as usize];
                Some(self.0 & ((1 << bits) - 1))
            }
            None => None,
        }
    }

    /// The NetID of the address (see the module notes for types 3 to 7).
    pub const fn net_id(self) -> Option<NetId> {
        match (self.net_type(), self.nwk_id()) {
            (Some(t), Some(n)) => Some(NetId(((t as u32) << 21) | n)),
            _ => None,
        }
    }

    /// The operator of the address's NetID.
    pub fn operator(self) -> Option<&'static str> {
        self.net_id().and_then(NetId::operator)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The "DevAddr Subnet" column of NetID Allocations R119 lists, for each NetID, the DevAddr prefix it owns.
    #[test]
    fn devaddr_subnets_of_the_allocation_list() {
        for (subnet, net_id, name) in [
            (0x0400_0000u32, 0x000002u32, "Actility"),
            (0x2600_0000, 0x000013, "The Things Network"),
            (0x7800_0000, 0x00003C, "The Helium Foundation"),
            (0x7E00_0000, 0x00003F, "Birdz"),
            (0xE002_0000, 0x600001, "Digita"),
            (0xE078_0000, 0x60003C, "SNCF SA"),
            (0xFC00_0800, 0xC00002, "ResIOT"),
            (
                0xFC01_4C00,
                0xC00053,
                "Decentralized Wireless Foundation Inc",
            ),
            (0xFC01_6000, 0xC00058, "The Things Network Foundation"),
            (0xFE00_1000, 0xE00020, "Techtenna b.v."),
        ] {
            let d = DevAddr(subnet | 0x1);
            assert_eq!(d.net_id(), Some(NetId(net_id)), "{subnet:08X}");
            assert_eq!(d.operator(), Some(name), "{subnet:08X}");
        }
    }

    #[test]
    fn type7_runs_and_unassigned() {
        assert_eq!(NetId(0xE0002F).operator(), Some("Techtenna b.v."));
        assert_eq!(NetId(0xE00030).operator(), Some("LNX Solutions"));
        assert_eq!(NetId(0x000005).operator(), None);
        assert_eq!(
            NetId(0x000000).operator(),
            Some("Private/experimental nodes")
        );
        assert_eq!(
            NetId(0x000040).operator(),
            None,
            "RFU bit set in a type 0 NetID"
        );
        assert!(!NetId(0x000040).is_valid() && NetId(0x6000FF).is_valid());
        assert_eq!(NetId(0x6000FF).net_type(), 3);
        assert_eq!(NetId(0x6000FF).id(), 0xFF, "the example of TS002");
        assert_eq!(DevAddr(0xFF00_0000).net_type(), None);
        assert_eq!(DevAddr(0x2601_1234).nwk_addr(), Some(0x0001_1234));
    }
}
