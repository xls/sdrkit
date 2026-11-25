//! The fields of the extended header that point to other packets (Bluetooth Core Specification v5.4 Vol 6 Part B clauses
//! 2.3.4.5 and 2.3.4.6): AuxPtr, which places the auxiliary packet of an extended advertising event, and SyncInfo, which
//! describes a periodic advertising train. Both are read and written as the clause's figures lay them out, least significant
//! bit first.

use super::phy::Phy;

/// The unit of an offset field: 30 us (0) or 300 us (1) (tables 2.15 and 2.18).
fn unit_s(units_300: bool) -> f64 {
    if units_300 {
        300e-6
    } else {
        30e-6
    }
}

/// The AuxPtr field (figure 2.19): Channel Index (6 bits), CA (1), Offset Units (1), AUX Offset (13), AUX PHY (3).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct AuxPtr {
    /// The channel index of the auxiliary packet (0 to 39; 0 to 36 on the secondary advertising physical channel).
    pub channel: u8,
    /// CA: the advertiser's clock is within 50 ppm.
    pub clock_50ppm: bool,
    /// Offset Units: 300 us when set, else 30 us.
    pub units_300us: bool,
    /// AUX Offset in its units.
    pub offset: u16,
    /// AUX PHY code (table 2.16: 0 LE 1M, 1 LE 2M, 2 LE Coded).
    pub phy_code: u8,
}

impl AuxPtr {
    /// Reads the three octets.
    pub fn read(b: [u8; 3]) -> AuxPtr {
        let v = b[0] as u32 | (b[1] as u32) << 8 | (b[2] as u32) << 16;
        AuxPtr {
            channel: (v & 0x3F) as u8,
            clock_50ppm: v & 0x40 != 0,
            units_300us: v & 0x80 != 0,
            offset: ((v >> 8) & 0x1FFF) as u16,
            phy_code: (v >> 21) as u8 & 7,
        }
    }

    /// The three octets.
    pub fn bytes(&self) -> [u8; 3] {
        let v = (self.channel as u32 & 0x3F)
            | (self.clock_50ppm as u32) << 6
            | (self.units_300us as u32) << 7
            | (self.offset as u32 & 0x1FFF) << 8
            | (self.phy_code as u32 & 7) << 21;
        [v as u8, (v >> 8) as u8, (v >> 16) as u8]
    }

    /// The PHY of the auxiliary packet; `None` for a reserved code.
    pub fn phy(&self) -> Option<Phy> {
        Phy::from_code(self.phy_code)
    }

    /// The time from the start of the packet holding this field to the earliest start of the auxiliary packet, s. The packet
    /// starts no later than one unit after it. 0 means no auxiliary packet follows (clause 2.3.4.5).
    pub fn offset_s(&self) -> f64 {
        self.offset as f64 * unit_s(self.units_300us)
    }

    /// The window the auxiliary packet starts in after its reference, s: one offset unit.
    pub fn window_s(&self) -> f64 {
        unit_s(self.units_300us)
    }
}

/// The SyncInfo field (figure 2.21): Offset Base (13 bits), Offset Units (1), Offset Adjust (1), RFU (1), Interval (2 octets),
/// ChM (37 bits), SCA (3), AA (4 octets), CRCInit (3 octets), PeriodicEventCounter (2 octets).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct SyncInfo {
    /// Offset Base in its units.
    pub offset_base: u16,
    /// Offset Units: 300 us when set, else 30 us.
    pub units_300us: bool,
    /// Offset Adjust: 2.4576 s added.
    pub offset_adjust: bool,
    /// Interval, units of 1.25 ms.
    pub interval: u16,
    /// ChM: bit n of the 37 bits is channel n.
    pub channel_map: [u8; 5],
    /// SCA code (3 bits).
    pub sca: u8,
    /// The access address of the train.
    pub access_address: u32,
    /// CRCInit of the train.
    pub crc_init: u32,
    /// paEventCounter of the AUX_SYNC_IND that this field describes.
    pub event_counter: u16,
}

impl SyncInfo {
    /// The length of the field, octets.
    pub const LEN: usize = 18;

    /// Reads the 18 octets.
    pub fn read(b: &[u8; 18]) -> SyncInfo {
        let o = u16::from_le_bytes([b[0], b[1]]);
        let channel_map = [b[4], b[5], b[6], b[7], b[8] & 0x1F];
        SyncInfo {
            offset_base: o & 0x1FFF,
            units_300us: o & 0x2000 != 0,
            offset_adjust: o & 0x4000 != 0,
            interval: u16::from_le_bytes([b[2], b[3]]),
            channel_map,
            sca: b[8] >> 5,
            access_address: u32::from_le_bytes([b[9], b[10], b[11], b[12]]),
            crc_init: b[13] as u32 | (b[14] as u32) << 8 | (b[15] as u32) << 16,
            event_counter: u16::from_le_bytes([b[16], b[17]]),
        }
    }

    /// The 18 octets.
    pub fn bytes(&self) -> [u8; 18] {
        let o = (self.offset_base & 0x1FFF)
            | (self.units_300us as u16) << 13
            | (self.offset_adjust as u16) << 14;
        let mut b = [0u8; 18];
        b[..2].copy_from_slice(&o.to_le_bytes());
        b[2..4].copy_from_slice(&self.interval.to_le_bytes());
        b[4..9].copy_from_slice(&self.channel_map);
        b[8] = (self.channel_map[4] & 0x1F) | (self.sca & 7) << 5;
        b[9..13].copy_from_slice(&self.access_address.to_le_bytes());
        b[13] = self.crc_init as u8;
        b[14] = (self.crc_init >> 8) as u8;
        b[15] = (self.crc_init >> 16) as u8;
        b[16..18].copy_from_slice(&self.event_counter.to_le_bytes());
        b
    }

    /// syncPacketWindowOffset: the time from the start of the packet holding this field to the earliest start of the
    /// AUX_SYNC_IND it describes, s. 0 means the next one is further away than the field can say.
    pub fn offset_s(&self) -> f64 {
        self.offset_base as f64 * unit_s(self.units_300us)
            + if self.offset_adjust { 2.4576 } else { 0.0 }
    }

    /// The window the AUX_SYNC_IND starts in after its offset, s: one offset unit.
    pub fn window_s(&self) -> f64 {
        unit_s(self.units_300us)
    }

    /// The periodic advertising interval, s.
    pub fn interval_s(&self) -> f64 {
        self.interval as f64 * 1.25e-3
    }
}
