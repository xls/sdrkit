//! The advertising layouts of the Tile, Chipolo and Samsung SmartTag families: their 16-bit service UUIDs, the mode that
//! each UUID tells, and the 20-byte service data value of a registered SmartTag. Rolling identifiers and signatures stay
//! opaque bytes.

use sdrkit_core::Choice as _;

/// The UUID of an activated Tile.
pub const TILE_UUID: u16 = 0xFEED;
/// The UUID of a Tile in shipping mode, before activation.
pub const TILE_SHIPPING_UUID: u16 = 0xFEEC;
/// A further UUID assigned to Tile, with no published use.
pub const TILE_OTHER_UUID: u16 = 0xFD84;
/// The UUID of a Chipolo on its own network, disconnected from its owner.
pub const CHIPOLO_UUID: u16 = 0xFE33;
/// The UUID of a registered SmartTag.
pub const SMARTTAG_UUID: u16 = 0xFD5A;
/// The UUID of an unregistered SmartTag.
pub const SMARTTAG_UNREGISTERED_UUID: u16 = 0xFD59;
/// The bytes of the service data value of a registered SmartTag.
pub const SMARTTAG_LEN: usize = 20;
/// The version in the high nibble of byte 0 of the SmartTag value.
pub const SMARTTAG_VERSION: u8 = 1;
/// The largest aging counter: 3 bytes.
pub const AGING_MAX: u32 = 0x00FF_FFFF;
/// The UTC, s, at which the aging counter is 0.
pub const AGING_EPOCH_S: i64 = 1_593_648_000;
/// One step of the aging counter, s.
pub const AGING_STEP_S: i64 = 900;
/// The most bytes of a service data value in a non-extended advertising PDU: 31 less the length, the type and the UUID.
pub const MAX_SERVICE_DATA: usize = 27;

sdrkit_core::choice_enum! {
    /// The mode that the advertised UUID of a Tile, Chipolo or SmartTag tells.
    pub enum TagMode {
        /// A Tile: activated (UUID 0xFEED).
        Activated = ("activated", "activated"),
        /// A Tile: shipping mode, not activated (UUID 0xFEEC).
        Shipping = ("shipping", "shipping mode"),
        /// A SmartTag: registered to an owner (UUID 0xFD5A).
        Registered = ("registered", "registered"),
        /// A SmartTag: not registered (UUID 0xFD59).
        Unregistered = ("unregistered", "unregistered"),
        /// A Chipolo: disconnected from its owner's phone (UUID 0xFE33).
        Disconnected = ("disconnected", "disconnected from its owner"),
    }
}

sdrkit_core::choice_enum! {
    /// The tag state of a SmartTag, bits 0x07 of byte 0: the values 1 to 6 in order.
    #[derive(Default)]
    pub enum SmartTagState {
        /// 1: just disconnected or rebooted.
        PrematureLost = ("premature_lost", "premature lost"),
        /// 2: lost, after 15 minutes premature lost.
        #[default]
        Lost = ("lost", "lost"),
        /// 3: overmature lost.
        OvermatureLost = ("overmature_lost", "overmature lost"),
        /// 4: paired with one device.
        Paired = ("paired", "paired with one device"),
        /// 5: connected to one device.
        ConnectedOne = ("connected_one", "connected to one device"),
        /// 6: connected to two devices.
        ConnectedTwo = ("connected_two", "connected to two devices"),
    }
}

impl SmartTagState {
    /// The state of the 3-bit code `code`; `None` for 0 and 7, which have no name.
    pub fn of_code(code: u8) -> Option<SmartTagState> {
        let i = code.checked_sub(1)?;
        SmartTagState::from_index(u16::from(i))
    }
    /// The 3-bit code, 1 to 6.
    pub fn code(self) -> u8 {
        self.index() as u8 + 1
    }
}

/// The service data value of a registered SmartTag (UUID 0xFD5A), 20 bytes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SmartTagAdvert {
    /// The tag state code, bits 0x07 of byte 0.
    pub state_code: u8,
    /// The aging counter, bytes 1 to 3 little-endian: 15-minute steps from [`AGING_EPOCH_S`].
    pub aging_counter: u32,
    /// The privacy ID, bytes 4 to 11: a rolling identifier, opaque.
    pub privacy_id: [u8; 8],
    /// The region ID, bits 0xF0 of byte 12.
    pub region: u8,
    /// The E2E flag, bit 0x08 of byte 12: finders must encrypt location reports.
    pub e2e: bool,
    /// The UWB flag, bit 0x04 of byte 12.
    pub uwb: bool,
    /// The battery code, bits 0x03 of byte 12, 0 to 3.
    pub battery: u8,
    /// The signature, bytes 16 to 19, opaque.
    pub signature: [u8; 4],
}

impl SmartTagAdvert {
    /// Reads `v`: exactly [`SMARTTAG_LEN`] bytes, version 1 and the advertisement type bit (0x08 of byte 0) clear.
    /// `None` for any other value. Bytes 13 to 15 are not checked.
    pub fn parse(v: &[u8]) -> Option<SmartTagAdvert> {
        let v: &[u8; SMARTTAG_LEN] = v.try_into().ok()?;
        if v[0] >> 4 != SMARTTAG_VERSION || v[0] & 0x08 != 0 {
            return None;
        }
        let mut privacy_id = [0; 8];
        privacy_id.copy_from_slice(&v[4..12]);
        let mut signature = [0; 4];
        signature.copy_from_slice(&v[16..20]);
        Some(SmartTagAdvert {
            state_code: v[0] & 0x07,
            aging_counter: u32::from_le_bytes([v[1], v[2], v[3], 0]),
            privacy_id,
            region: v[12] >> 4,
            e2e: v[12] & 0x08 != 0,
            uwb: v[12] & 0x04 != 0,
            battery: v[12] & 0x03,
            signature,
        })
    }
    /// The 20 bytes: version 1, the type bit clear, bytes 13 to 15 zero. Fields wider than their bits are cut to them.
    pub fn bytes(&self) -> [u8; SMARTTAG_LEN] {
        let mut v = [0; SMARTTAG_LEN];
        v[0] = SMARTTAG_VERSION << 4 | (self.state_code & 0x07);
        v[1..4].copy_from_slice(&self.aging_counter.to_le_bytes()[..3]);
        v[4..12].copy_from_slice(&self.privacy_id);
        v[12] = (self.region & 0x0F) << 4
            | u8::from(self.e2e) << 3
            | u8::from(self.uwb) << 2
            | (self.battery & 0x03);
        v[16..20].copy_from_slice(&self.signature);
        v
    }
    /// The named tag state; `None` for the codes 0 and 7.
    pub fn state(&self) -> Option<SmartTagState> {
        SmartTagState::of_code(self.state_code)
    }
}

/// A copy of one service data value, up to [`MAX_SERVICE_DATA`] bytes; longer values keep their first bytes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ServiceData {
    len: u8,
    bytes: [u8; MAX_SERVICE_DATA],
}

impl ServiceData {
    /// A copy of `v`.
    pub fn of(v: &[u8]) -> ServiceData {
        let n = v.len().min(MAX_SERVICE_DATA);
        let mut bytes = [0; MAX_SERVICE_DATA];
        bytes[..n].copy_from_slice(&v[..n]);
        ServiceData {
            len: n as u8,
            bytes,
        }
    }
    /// The bytes.
    pub fn as_slice(&self) -> &[u8] {
        &self.bytes[..usize::from(self.len)]
    }
}

/// What an advertisement of these families carries beyond its kind.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TagDetail {
    /// The mode that the UUID tells.
    pub mode: Option<TagMode>,
    /// The decoded SmartTag value.
    pub smarttag: Option<SmartTagAdvert>,
    /// The service data value under the family's UUID, as sent.
    pub service_data: Option<ServiceData>,
}

impl TagDetail {
    /// Fills each part that is empty here from `o`.
    pub fn merge(&mut self, o: &TagDetail) {
        self.mode = self.mode.or(o.mode);
        self.smarttag = self.smarttag.or(o.smarttag);
        self.service_data = self.service_data.or(o.service_data);
    }
}

/// The mode that the 16-bit UUID `u` tells, for the UUIDs of these families that have one.
pub fn mode_of(u: u16) -> Option<TagMode> {
    match u {
        TILE_UUID => Some(TagMode::Activated),
        TILE_SHIPPING_UUID => Some(TagMode::Shipping),
        SMARTTAG_UUID => Some(TagMode::Registered),
        SMARTTAG_UNREGISTERED_UUID => Some(TagMode::Unregistered),
        CHIPOLO_UUID => Some(TagMode::Disconnected),
        _ => None,
    }
}

/// The detail of service data `v` under the UUID `u` of one of these families.
pub fn detail_of_service(u: u16, v: &[u8]) -> TagDetail {
    TagDetail {
        mode: mode_of(u),
        smarttag: if u == SMARTTAG_UUID {
            SmartTagAdvert::parse(v)
        } else {
            None
        },
        service_data: Some(ServiceData::of(v)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A registered SmartTag value as printed in the published analysis of the first SmartTag.
    const EXAMPLE: [u8; 20] = [
        0x15, 0x6B, 0xFA, 0x00, 0xC8, 0x40, 0x62, 0xB2, 0x8F, 0x00, 0xE2, 0x60, 0xC3, 0x00, 0x00,
        0x00, 0xAD, 0x01, 0x8B, 0x47,
    ];

    #[test]
    fn the_printed_smarttag_value_decodes_to_its_fields() {
        let s = SmartTagAdvert::parse(&EXAMPLE).unwrap();
        assert_eq!(s.state_code, 5);
        assert_eq!(s.state(), Some(SmartTagState::ConnectedOne));
        assert_eq!(s.aging_counter, 64_107);
        assert_eq!(
            s.privacy_id,
            [0xC8, 0x40, 0x62, 0xB2, 0x8F, 0x00, 0xE2, 0x60]
        );
        assert_eq!(s.region, 0xC);
        assert!(!s.e2e && !s.uwb);
        assert_eq!(s.battery, 3);
        assert_eq!(s.signature, [0xAD, 0x01, 0x8B, 0x47]);
        // 64,107 steps of 15 minutes from 2020-07-02 is late April 2022.
        let utc = AGING_EPOCH_S + i64::from(s.aging_counter) * AGING_STEP_S;
        assert!((1_650_700_000..1_651_400_000).contains(&utc), "{utc}");
    }

    #[test]
    fn the_fields_of_the_printed_value_build_its_bytes() {
        let s = SmartTagAdvert {
            state_code: SmartTagState::ConnectedOne.code(),
            aging_counter: 64_107,
            privacy_id: [0xC8, 0x40, 0x62, 0xB2, 0x8F, 0x00, 0xE2, 0x60],
            region: 0xC,
            e2e: false,
            uwb: false,
            battery: 3,
            signature: [0xAD, 0x01, 0x8B, 0x47],
        };
        assert_eq!(s.bytes(), EXAMPLE);
    }

    #[test]
    fn every_bit_of_byte_12_and_the_state_go_both_ways() {
        for (region, e2e, uwb, battery, state) in [(0, true, false, 0, 1), (15, false, true, 2, 6)]
        {
            let s = SmartTagAdvert {
                state_code: state,
                aging_counter: AGING_MAX,
                region,
                e2e,
                uwb,
                battery,
                ..SmartTagAdvert::default()
            };
            assert_eq!(SmartTagAdvert::parse(&s.bytes()), Some(s));
        }
    }

    #[test]
    fn other_lengths_versions_and_types_are_not_decoded() {
        assert!(SmartTagAdvert::parse(&EXAMPLE[..19]).is_none());
        let mut v = EXAMPLE;
        v[0] = 0x25;
        assert!(SmartTagAdvert::parse(&v).is_none());
        v[0] = 0x1D;
        assert!(SmartTagAdvert::parse(&v).is_none());
    }

    #[test]
    fn the_codes_without_a_name_have_no_state() {
        assert_eq!(SmartTagState::of_code(0), None);
        assert_eq!(SmartTagState::of_code(7), None);
        for c in 1..=6 {
            assert_eq!(SmartTagState::of_code(c).map(SmartTagState::code), Some(c));
        }
    }
}
