//! Bluetooth assigned numbers: company identifiers (the first two bytes of manufacturer specific data, Core Specification
//! Supplement Part A clause 1.4) and 16-bit UUIDs (GATT services and the 16-bit UUIDs assigned to SIG members).
//!
//! **Data.** `data/bt_company.tsv` and `data/bt_uuid16.tsv` hold the whole of the Bluetooth SIG "Assigned Numbers" lists
//! `company_identifiers/company_identifiers.yaml`, `uuids/service_uuids.yaml` and `uuids/member_uuids.yaml` from
//! <https://bitbucket.org/bluetooth-SIG/public/src/main/assigned_numbers/> (company list of 5 November 2026, retrieved
//! 8 November 2026). Names are as the SIG publishes them.
//!
//! **Update:**
//!
//! ```text
//! B=https://bitbucket.org/bluetooth-SIG/public/raw/main/assigned_numbers
//! curl -O $B/company_identifiers/company_identifiers.yaml -O $B/uuids/service_uuids.yaml -O $B/uuids/member_uuids.yaml
//! cargo run -p sdrkit-util-ids --example update_tables -- bt-company company_identifiers.yaml > crates/util/sdrkit-util-ids/data/bt_company.tsv
//! cargo run -p sdrkit-util-ids --example update_tables -- bt-uuid service_uuids.yaml member_uuids.yaml \
//!     > crates/util/sdrkit-util-ids/data/bt_uuid16.tsv
//! ```
//!
//! ```
//! use sdrkit_util_ids::bluetooth::{self, UuidKind};
//!
//! assert_eq!(bluetooth::company(0x004C), Some("Apple, Inc."));
//! let u = bluetooth::uuid16(0x180F).unwrap();
//! assert_eq!((u.name, u.kind), ("Battery", UuidKind::Service));
//! ```

use crate::pool::{self, find};

/// The company of a company identifier.
pub fn company(id: u16) -> Option<&'static str> {
    find(&pool::BT_COMPANY_KEYS, &id).map(|i| pool::BT_COMPANY_POOL.get(pool::BT_COMPANY_NAME[i]))
}

/// What a 16-bit UUID is assigned to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum UuidKind {
    /// A GATT service defined by the SIG.
    Service,
    /// A UUID the SIG assigned to a member company (0xFCxx to 0xFExx); the name is the company.
    Member,
}

/// A named 16-bit UUID.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Uuid16 {
    /// The service name, or the member company.
    pub name: &'static str,
    /// Service or member UUID.
    pub kind: UuidKind,
}

/// What a 16-bit UUID is assigned to.
pub fn uuid16(uuid: u16) -> Option<Uuid16> {
    find(&pool::BT_UUID_KEYS, &uuid).map(|i| Uuid16 {
        name: pool::BT_UUID_POOL.get(pool::BT_UUID_NAME[i]),
        kind: if pool::BT_UUID_KIND[i] == 0 {
            UuidKind::Service
        } else {
            UuidKind::Member
        },
    })
}

/// The Bluetooth Base UUID 00000000-0000-1000-8000-00805F9B34FB (Core Specification Vol 3 Part B clause 2.5.1), most
/// significant byte first.
pub const BASE_UUID: [u8; 16] = [
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x10, 0x00, 0x80, 0x00, 0x00, 0x80, 0x5F, 0x9B, 0x34, 0xFB,
];

/// The 16-bit UUID a 128-bit UUID (most significant byte first) stands for, when it is the Base UUID with a 16-bit value in
/// bytes 2 and 3. Advertising data carries 128-bit UUIDs least significant byte first: reverse them before calling.
pub fn short_uuid(uuid128: &[u8; 16]) -> Option<u16> {
    let same = uuid128[..2] == [0, 0] && uuid128[4..] == BASE_UUID[4..];
    same.then(|| u16::from_be_bytes([uuid128[2], uuid128[3]]))
}

/// The number of company identifiers in the table.
pub fn company_count() -> usize {
    pool::BT_COMPANY_KEYS.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn companies_from_the_sig_list() {
        assert_eq!(company(0x0000), Some("Ericsson AB"));
        assert_eq!(company(0x0006), Some("Microsoft"));
        assert_eq!(company(0x0059), Some("Nordic Semiconductor ASA"));
        assert_eq!(
            company(0x02E5),
            Some("Espressif Systems (Shanghai) Co., Ltd.")
        );
        assert!(company_count() > 3900);
    }

    #[test]
    fn uuids_from_the_sig_lists() {
        assert_eq!(uuid16(0x1800).unwrap().name, "GAP");
        assert_eq!(uuid16(0x180D).unwrap().name, "Heart Rate");
        let m = uuid16(0xFEAA).unwrap();
        assert_eq!((m.name, m.kind), ("Google LLC", UuidKind::Member));
        assert_eq!(uuid16(0x0001), None);
    }

    #[test]
    fn base_uuid() {
        let mut u = BASE_UUID;
        u[2] = 0x18;
        u[3] = 0x0F;
        assert_eq!(short_uuid(&u), Some(0x180F));
        u[15] ^= 1;
        assert_eq!(short_uuid(&u), None);
    }
}
