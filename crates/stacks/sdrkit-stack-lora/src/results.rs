//! The choices of the result sets of the family and the family of the frames of a protocol.

use crate::mesh::types::Protocol;
use sdrkit_core::publish::keys;

sdrkit_core::choice_enum! {
    /// How a frame was modulated.
    pub enum Modulation {
        /// LoRa chirps.
        Lora = ("lora", "LoRa"),
        /// FSK (the LoRaWAN FSK channel).
        Fsk = ("fsk", "FSK"),
    }
}

sdrkit_core::choice_enum! {
    /// What the CRC of a frame says.
    pub enum CrcState {
        /// The CRC is right.
        Ok = ("ok", "Right"),
        /// The CRC is wrong.
        Bad = ("bad", "Wrong"),
        /// The frame has no CRC.
        None = ("none", "None"),
    }
}

/// The family of the frames of a protocol.
pub fn family(p: Protocol) -> keys::Family {
    match p {
        Protocol::Meshtastic | Protocol::MeshCore | Protocol::MeshCom => keys::Family::Message,
        Protocol::LoraAprs | Protocol::Aprs => keys::Family::Aprs,
        Protocol::Lorawan => keys::Family::Sensor,
        Protocol::Lora => keys::Family::Other,
    }
}
