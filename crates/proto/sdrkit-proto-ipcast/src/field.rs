//! The data field of one DVB-S2 BBFRAME as a graph item, for receivers that read a generic stream byte by byte instead of
//! its packets (CADUs carried in the data field, for example).
//!
//! Standard: ETSI EN 302 307-1 clause 5.1.6.

use crate::bbheader::{BbHeader, BBHEADER_LEN};
use sdrkit_graph::{Item, Packet};

/// One good BBFRAME (BCH good, BBHEADER CRC-8 good): the `dvbs2.data_field` item.
#[derive(Clone, Debug, PartialEq)]
pub struct DataField {
    /// The 10 BBHEADER bytes as received (after BB descrambling).
    pub raw_header: [u8; BBHEADER_LEN],
    /// The decoded BBHEADER: TS/GS, SIS/MIS, CCM/ACM, ISSYI, NPD, RO, ISI, UPL, DFL, SYNC, SYNCD.
    pub header: BbHeader,
    /// The data field: DFL / 8 bytes.
    pub data: Packet,
    /// The MODCOD of the PLFRAME (1 to 28 for DVB-S2).
    pub modcod: u8,
    /// Short FEC frame (16 200 bits).
    pub short: bool,
    /// The PLFRAME had pilots.
    pub pilots: bool,
    /// BBFRAMEs lost just before this one: BCH failures and bad BBHEADERs. A consumer that joins data fields into one
    /// stream breaks the stream when it is not 0.
    pub lost_before: u64,
    /// Signal time of the frame, seconds.
    pub time: f64,
}

impl Item for DataField {
    const KIND: &'static str = "dvbs2.data_field";
}

impl DataField {
    /// The data field bytes.
    pub fn bytes(&self) -> &[u8] {
        self.data.bytes()
    }
}
