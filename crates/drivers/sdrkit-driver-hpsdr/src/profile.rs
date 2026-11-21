//! The four radio profiles: one implementation, four kinds of radio that differ only by these numbers. The board ids, board
//! names and rate lists are facts of the protocol documents.
//! Standard: HPSDR USB data protocol (protocol 1), openHPSDR Ethernet protocol (protocol 2).

use sdrkit_core::Hz;

/// The kind of radio a profile is for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Model {
    /// The Hermes-Lite 2.
    HermesLite2,
    /// A Red Pitaya that runs the HPSDR-compatible transceiver program.
    RedPitaya,
    /// Another protocol 1 board.
    Hpsdr1,
    /// Protocol 2 firmware.
    Hpsdr2,
}

/// Which kind of gain control a profile has.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GainKind {
    /// The Hermes-Lite 2 LNA, -12 to +48 dB, register 0x0a.
    Lna,
    /// A step attenuator, offered as a gain of -31 to 0 dB.
    Attenuator,
    /// A gain of 0 to 40 dB that the driver applies to the sample words (the Red Pitaya program has no analogue gain).
    Words,
}

/// The filter board a profile drives.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FilterBoard {
    /// The N2ADR board on the open-collector outputs (Hermes-Lite 2).
    N2adr,
    /// None.
    None,
    /// The radio's own Alex board, chosen by its firmware (protocol 1).
    AlexAuto,
    /// The Alex board, set by sdrkit (protocol 2).
    AlexHost,
}

/// One radio profile.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Profile {
    /// The kind of radio.
    pub model: Model,
    /// The key of the driver kind.
    pub key: &'static str,
    /// The name in messages and in the radio list.
    pub name: &'static str,
    /// The protocol, 1 or 2.
    pub protocol: u8,
    /// Board ids accepted in a discovery reply.
    pub boards: &'static [u8],
    /// The ADC clock.
    pub clock_hz: Hz,
    /// The lowest frequency offered.
    pub min_freq_hz: Hz,
    /// The highest frequency offered.
    pub max_freq_hz: Hz,
    /// Receivers when the radio did not say (protocol 1 HPSDR: by board, see [`Profile::default_receivers`]).
    pub default_receivers: u8,
    /// The most receivers.
    pub max_receivers: u8,
    /// The gain control.
    pub gain: GainKind,
    /// The filter board.
    pub filter: FilterBoard,
}

/// Protocol 1 receive rates.
pub const RATES_P1: [u32; 4] = [48_000, 96_000, 192_000, 384_000];
/// Protocol 2 receive rates on the boards that have them all.
pub const RATES_P2_FULL: [u32; 6] = [48_000, 96_000, 192_000, 384_000, 768_000, 1_536_000];

/// The Hermes-Lite 2.
pub const HERMES_LITE2: Profile = Profile {
    model: Model::HermesLite2,
    key: "hermeslite2",
    name: "Hermes Lite 2",
    protocol: 1,
    boards: &[6],
    clock_hz: 76.8e6,
    min_freq_hz: 10e3,
    max_freq_hz: 38.4e6,
    default_receivers: 4,
    max_receivers: 12,
    gain: GainKind::Lna,
    filter: FilterBoard::N2adr,
};

/// A Red Pitaya running the "SDR transceiver compatible with HPSDR" program.
pub const RED_PITAYA: Profile = Profile {
    model: Model::RedPitaya,
    key: "redpitaya",
    name: "Red Pitaya",
    protocol: 1,
    boards: &[1],
    clock_hz: 122.88e6,
    min_freq_hz: 10e3,
    max_freq_hz: 61.44e6,
    default_receivers: 3,
    max_receivers: 3,
    gain: GainKind::Words,
    filter: FilterBoard::None,
};

/// Other protocol 1 boards.
pub const HPSDR1: Profile = Profile {
    model: Model::Hpsdr1,
    key: "hpsdr1",
    name: "HPSDR",
    protocol: 1,
    boards: &[0, 1, 2, 4, 5, 10],
    clock_hz: 122.88e6,
    min_freq_hz: 10e3,
    max_freq_hz: 61.44e6,
    default_receivers: 4,
    max_receivers: 7,
    gain: GainKind::Attenuator,
    filter: FilterBoard::AlexAuto,
};

/// Protocol 2 firmware (ANAN, Orion).
pub const HPSDR2: Profile = Profile {
    model: Model::Hpsdr2,
    key: "hpsdr2",
    name: "HPSDR",
    protocol: 2,
    boards: &[0, 1, 2, 3, 4, 5, 6],
    clock_hz: 122.88e6,
    min_freq_hz: 10e3,
    max_freq_hz: 61.44e6,
    default_receivers: 2,
    max_receivers: 80,
    gain: GainKind::Attenuator,
    filter: FilterBoard::AlexHost,
};

/// Every profile, in the order of the driver list.
pub const ALL: [Profile; 4] = [HERMES_LITE2, RED_PITAYA, HPSDR1, HPSDR2];

impl Profile {
    /// The board id is one this profile accepts.
    pub fn accepts(&self, board: u8) -> bool {
        self.boards.contains(&board)
    }

    /// Receivers when the radio said nothing, for a board seen (or not) at discovery.
    pub fn default_receivers(&self, board: Option<u8>) -> u8 {
        if self.model == Model::Hpsdr1 {
            match board {
                Some(2) => 2,
                Some(4 | 5 | 10) => 7,
                _ => 4,
            }
        } else {
            self.default_receivers
        }
    }

    /// The receive rates offered on `board` (on protocol 2 they depend on the board, and an unknown board provides the full list).
    pub fn rates(&self, board: Option<u8>) -> &'static [u32] {
        if self.protocol == 1 {
            &RATES_P1
        } else {
            match board {
                Some(1 | 2 | 6) => &RATES_P2_FULL[..4],
                _ => &RATES_P2_FULL,
            }
        }
    }

    /// The board has a gain control (Metis, board 0 on protocol 1, has no attenuator).
    pub fn has_gain(&self, board: Option<u8>) -> bool {
        !(self.model == Model::Hpsdr1 && board == Some(0))
    }

    /// The gain range offered (min, max, step) in dB.
    pub fn gain_range(&self, board: Option<u8>) -> Option<(f64, f64, f64)> {
        if !self.has_gain(board) {
            return None;
        }
        Some(match self.gain {
            GainKind::Lna => (-12.0, 48.0, 1.0),
            GainKind::Attenuator => (-31.0, 0.0, 1.0),
            GainKind::Words => (0.0, 40.0, 1.0),
        })
    }

    /// The protocol 2 receive-specific packet's ADC count for a board.
    pub fn adcs(board: Option<u8>) -> u8 {
        match board {
            Some(3..=5) => 2,
            _ => 1,
        }
    }
}

/// A board's name, by protocol.
pub fn board_name(protocol: u8, board: u8) -> &'static str {
    if protocol == 1 {
        match board {
            0 => "Metis",
            1 => "Hermes",
            2 => "Griffin",
            4 => "Angelia",
            5 => "Orion",
            6 => "Hermes-Lite 2",
            10 => "Orion MkII",
            _ => "unknown board",
        }
    } else {
        match board {
            0 => "Atlas",
            1 => "Hermes",
            2 => "Hermes II",
            3 => "Angelia",
            4 => "Orion",
            5 => "Orion MkII",
            6 => "Hermes-Lite",
            _ => "unknown board",
        }
    }
}
