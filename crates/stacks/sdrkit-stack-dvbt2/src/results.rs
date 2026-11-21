//! The values that the results of the DVB-T2 stack show. The shapes of the results are in [`crate::typed`].

use sdrkit_proto_mpegts::layer::TsLayer;

/// The parts of the transport stream layer that the stack adds: services, tracks, guide, teletext and subtitles.
pub const TS_LAYER: TsLayer = TsLayer {
    teletext: true,
    subtitles: true,
};

sdrkit_core::choice_enum! {
    /// How far the receiver has come.
    pub enum Detection {
        /// Looking for P1.
        Searching = ("searching", "Searching"),
        /// P1 found, L1 not decoded yet.
        P1 = ("p1", "P1 found"),
        /// L1 decoded, no FEC block accepted yet.
        L1 = ("l1", "L1 decoded"),
        /// Transport packets flow.
        Flowing = ("flowing", "Flowing"),
    }
}

sdrkit_core::choice_enum! {
    /// PLP_TYPE (EN 302 755 clause 7.2.3.1).
    pub enum PlpType {
        /// The common PLP of a group.
        Common = ("common", "Common"),
        /// Data PLP type 1.
        Data1 = ("data1", "Data type 1"),
        /// Data PLP type 2.
        Data2 = ("data2", "Data type 2"),
        /// A reserved value.
        Reserved = ("reserved", "Reserved"),
    }
}

impl PlpType {
    /// The type of a PLP_TYPE code.
    pub fn of(code: u8) -> PlpType {
        match code {
            0 => PlpType::Common,
            1 => PlpType::Data1,
            2 => PlpType::Data2,
            _ => PlpType::Reserved,
        }
    }
}

sdrkit_core::choice_enum! {
    /// PLP_PAYLOAD_TYPE (EN 302 755 clause 7.2.3.1).
    pub enum PayloadType {
        /// Generic fixed-length packetized stream.
        Gfps = ("gfps", "GFPS"),
        /// Generic continuous stream.
        Gcs = ("gcs", "GCS"),
        /// Generic stream encapsulation.
        Gse = ("gse", "GSE"),
        /// Transport stream.
        Ts = ("ts", "TS"),
        /// A reserved value.
        Reserved = ("reserved", "Reserved"),
    }
}

impl PayloadType {
    /// The payload type of a PLP_PAYLOAD_TYPE code.
    pub fn of(code: u8) -> PayloadType {
        match code {
            0 => PayloadType::Gfps,
            1 => PayloadType::Gcs,
            2 => PayloadType::Gse,
            3 => PayloadType::Ts,
            _ => PayloadType::Reserved,
        }
    }
}

sdrkit_core::choice_enum! {
    /// How a PLP is time interleaved (TIME_IL_TYPE and TIME_IL_LENGTH, EN 302 755 clause 7.2.3.1).
    pub enum TimeIl {
        /// No time interleaving.
        Off = ("off", "Off"),
        /// TIME_IL_LENGTH TI-blocks in each interleaving frame.
        Blocks = ("blocks", "TI-blocks per frame"),
        /// One TI-block over TIME_IL_LENGTH T2-frames.
        Frames = ("frames", "Over T2-frames"),
    }
}
