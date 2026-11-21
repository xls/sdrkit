//! The profiles of `eodb`: the fixed signal constants of each direct-broadcast or dump link (modulation, symbol rate, codes,
//! frame, header, packet zone, virtual channel and instrument). No public document confirms these constants; the satellites
//! and their downlinks are data, not part of a profile.

use crate::psk::Modulation;

/// Differential coding.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Diff {
    /// None.
    None,
    /// NRZ-M on the data bit stream (before the K = 7 code, or on the whole stream of an uncoded link).
    NrzM,
    /// NRZ-M on each of the I and Q rails before they are interleaved.
    PerRail,
    /// The quadrant difference of Oceansat-2.
    Dqpsk1,
    /// The four-phase rule of Kanopus-V.
    FourPhase,
}

/// The inner code.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Conv {
    /// None.
    None,
    /// The K = 7 rate 1/2 code of CCSDS: G1 then G2 inverted.
    Ccsds,
    /// The same code with the two code bits of each pair in the other order.
    Swapped,
    /// The CCSDS (8160, 7136) LDPC code: not decoded.
    Ldpc,
    /// The CCSDS turbo code: not decoded.
    Turbo,
}

/// Where the CCSDS randomiser sits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rand {
    /// No randomiser.
    None,
    /// Over every byte after the marker: removed before the Reed-Solomon decoding.
    Before,
    /// Over the data inside the Reed-Solomon code: removed after it.
    After,
}

/// The frame header.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Header {
    /// AOS with an insert zone of this many bytes.
    Aos(usize),
    /// TM.
    Tm,
    /// No CCSDS frame.
    None,
}

/// The instrument whose packets or frames a profile decodes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Instrument {
    /// None: frames only.
    None,
    /// VIIRS.
    Viirs,
    /// MODIS.
    Modis,
    /// AVHRR of the MetOp dump.
    Avhrr,
    /// METimage.
    Metimage,
    /// TIRS.
    Tirs,
    /// VEGETATION.
    Vegetation,
    /// AMSR2.
    Amsr2,
    /// Oceansat-2 OCM.
    Ocm,
    /// Kanopus-V PSS and MSS.
    Kanopus,
    /// DMSP OLS.
    Ols,
}

/// The Reed-Solomon code of a profile.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RsParams {
    /// Correctable symbols E: 16 for (255,223), 8 for (255,239).
    pub e: usize,
    /// Interleave depth.
    pub interleave: usize,
    /// Virtual fill.
    pub fill: usize,
}

/// One link.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Profile {
    /// The submode id.
    pub id: &'static str,
    /// The link name of the summary.
    pub name: &'static str,
    /// The modulation.
    pub modulation: Modulation,
    /// Symbols a second.
    pub symbol_rate: f64,
    /// The root raised cosine roll-off.
    pub roll_off: f64,
    /// Differential coding.
    pub diff: Diff,
    /// The inner code.
    pub conv: Conv,
    /// The Reed-Solomon code.
    pub rs: Option<RsParams>,
    /// The randomiser.
    pub rand: Rand,
    /// Every byte after the marker is inverted before anything else.
    pub invert: bool,
    /// A frame that fails is tried again without the randomiser.
    pub retry_plain: bool,
    /// Bytes of a CADU with its marker (0 for the profiles without CADUs).
    pub frame: usize,
    /// The frame header.
    pub header: Header,
    /// Bytes of the packet zone (0: frames only).
    pub zone: usize,
    /// The virtual channel whose packets are read.
    pub vc: Option<u8>,
    /// The instrument.
    pub instrument: Instrument,
    /// The input rate asked, samples a second.
    pub rate: f64,
    /// The least input rate.
    pub least: f64,
    /// The spacecraft id of the test signal.
    pub test_spacecraft: u8,
}

impl Profile {
    /// Check bytes of a code word, 2E.
    pub fn check_bytes(&self) -> usize {
        self.rs.map_or(0, |r| 2 * r.e)
    }
    /// Bytes of the frame after decoding: frame - 4 - 2E I.
    pub fn data_bytes(&self) -> usize {
        let i = self.rs.map_or(0, |r| r.interleave);
        self.frame.saturating_sub(4 + self.check_bytes() * i)
    }
    /// Bytes of the frame header before the M_PDU header: 6 plus the insert zone for AOS, 6 for TM.
    pub fn header_bytes(&self) -> usize {
        match self.header {
            Header::Aos(n) => 6 + n,
            Header::Tm => 6,
            Header::None => 0,
        }
    }
    /// The first byte of the packet zone in the frame after decoding (the marker not counted).
    pub fn zone_at(&self) -> usize {
        match self.header {
            Header::Aos(n) => 6 + n + 2,
            Header::Tm => 6,
            Header::None => 0,
        }
    }
    /// Whether frames come from samples: the profile has CADUs and no LDPC or turbo code.
    pub fn from_samples(&self) -> bool {
        !matches!(self.conv, Conv::Ldpc | Conv::Turbo)
    }
    /// The profile has CADUs behind the marker 1ACFFC1D.
    pub fn has_cadus(&self) -> bool {
        self.frame > 0
    }
    /// Code symbols a channel symbol carries: 2 for QPSK and offset QPSK, else 1.
    pub fn bits_per_symbol(&self) -> usize {
        self.modulation.bits_per_symbol()
    }
    /// The coded rate: 2 with a K = 7 code, else 1.
    pub fn code_factor(&self) -> usize {
        match self.conv {
            Conv::Ccsds | Conv::Swapped => 2,
            _ => 1,
        }
    }
    /// Seconds from one CADU to the next.
    pub fn frame_period_s(&self) -> f64 {
        (8 * self.frame * self.code_factor()) as f64 / (self.symbol_rate * self.bits_per_symbol() as f64)
    }
    /// The bandwidth shown: 1.4 times the symbol rate.
    pub fn shown_hz(&self) -> f64 {
        1.4 * self.symbol_rate
    }
}

const fn rs(e: usize, interleave: usize, fill: usize) -> Option<RsParams> {
    Some(RsParams { e, interleave, fill })
}

const BASE: Profile = Profile {
    id: "",
    name: "",
    modulation: Modulation::Qpsk,
    symbol_rate: 1e6,
    roll_off: 0.5,
    diff: Diff::None,
    conv: Conv::None,
    rs: None,
    rand: Rand::Before,
    invert: false,
    retry_plain: false,
    frame: 1024,
    header: Header::Aos(0),
    zone: 0,
    vc: None,
    instrument: Instrument::None,
    rate: 1e6,
    least: 1e6,
    test_spacecraft: 1,
};

/// The 20 profiles, in the order of their numbers.
pub static PROFILES: [Profile; 20] = [
    Profile {
        id: "jpss_hrd",
        name: "JPSS HRD",
        symbol_rate: 15e6,
        diff: Diff::NrzM,
        conv: Conv::Ccsds,
        rs: rs(16, 4, 0),
        zone: 884,
        vc: Some(16),
        instrument: Instrument::Viirs,
        rate: 25e6,
        least: 18e6,
        test_spacecraft: 159,
        ..BASE
    },
    Profile {
        id: "jpss_hrd_n21",
        name: "JPSS HRD (NOAA-21)",
        modulation: Modulation::Oqpsk,
        symbol_rate: 25e6,
        diff: Diff::NrzM,
        conv: Conv::Ccsds,
        rs: rs(16, 5, 0),
        frame: 1279,
        header: Header::Aos(9),
        zone: 1094,
        vc: Some(16),
        instrument: Instrument::Viirs,
        rate: 40e6,
        least: 32e6,
        test_spacecraft: 177,
        ..BASE
    },
    Profile {
        id: "eos_aqua_aura",
        name: "EOS Aqua and Aura DB",
        modulation: Modulation::Oqpsk,
        symbol_rate: 7.5e6,
        diff: Diff::PerRail,
        rs: rs(16, 4, 0),
        zone: 884,
        vc: Some(30),
        instrument: Instrument::Modis,
        rate: 15e6,
        least: 10e6,
        test_spacecraft: 154,
        ..BASE
    },
    Profile {
        id: "eos_terra",
        name: "EOS Terra DB",
        modulation: Modulation::Bpsk,
        symbol_rate: 13.125e6,
        diff: Diff::NrzM,
        conv: Conv::Swapped,
        rs: rs(16, 4, 0),
        rand: Rand::After,
        zone: 884,
        vc: Some(42),
        instrument: Instrument::Modis,
        rate: 45e6,
        least: 30e6,
        test_spacecraft: 42,
        ..BASE
    },
    Profile {
        id: "metop_dump",
        name: "MetOp X-band dump",
        symbol_rate: 35e6,
        rs: rs(16, 4, 0),
        invert: true,
        header: Header::Aos(2),
        zone: 882,
        vc: Some(9),
        instrument: Instrument::Avhrr,
        rate: 45e6,
        least: 40e6,
        test_spacecraft: 11,
        ..BASE
    },
    Profile {
        id: "metopsg_ddb",
        name: "MetOp-SG direct data broadcast",
        symbol_rate: 93.3e6,
        conv: Conv::Swapped,
        rs: rs(16, 4, 0),
        zone: 884,
        vc: Some(16),
        instrument: Instrument::Metimage,
        rate: 140e6,
        least: 105e6,
        test_spacecraft: 3,
        ..BASE
    },
    Profile {
        id: "landsat_x",
        name: "Landsat X band",
        modulation: Modulation::Oqpsk,
        symbol_rate: 220.4e6,
        conv: Conv::Ldpc,
        rand: Rand::None,
        frame: 1034,
        zone: 1022,
        vc: Some(5),
        instrument: Instrument::Tirs,
        rate: 300e6,
        least: 240e6,
        ..BASE
    },
    Profile {
        id: "landsat_s",
        name: "Landsat S band",
        modulation: Modulation::Oqpsk,
        symbol_rate: 1.067e6,
        diff: Diff::NrzM,
        conv: Conv::Ccsds,
        rs: rs(16, 5, 0),
        frame: 1279,
        rate: 6e6,
        least: 2e6,
        ..BASE
    },
    Profile {
        id: "proba_s",
        name: "Proba S band",
        modulation: Modulation::Bpsk,
        symbol_rate: 2e6,
        conv: Conv::Swapped,
        rs: rs(16, 5, 0),
        retry_plain: true,
        frame: 1279,
        header: Header::Tm,
        zone: 1103,
        vc: Some(4),
        instrument: Instrument::Vegetation,
        rate: 4e6,
        least: 3e6,
        ..BASE
    },
    Profile {
        id: "proba_v_x",
        name: "Proba-V X band",
        modulation: Modulation::Oqpsk,
        symbol_rate: 35e6,
        conv: Conv::Ccsds,
        rs: rs(16, 5, 0),
        frame: 1279,
        header: Header::Tm,
        zone: 1103,
        rate: 45e6,
        least: 38e6,
        ..BASE
    },
    Profile {
        id: "gcom_w1",
        name: "GCOM-W1 X band",
        modulation: Modulation::Oqpsk,
        symbol_rate: 10e6,
        diff: Diff::NrzM,
        conv: Conv::Ccsds,
        rs: rs(8, 5, 3),
        frame: 1264,
        zone: 1092,
        vc: Some(17),
        instrument: Instrument::Amsr2,
        rate: 20e6,
        least: 13e6,
        ..BASE
    },
    Profile {
        id: "gcom_c1",
        name: "GCOM-C1 X band",
        modulation: Modulation::Oqpsk,
        symbol_rate: 69.416e6,
        diff: Diff::NrzM,
        frame: 1264,
        rate: 90e6,
        least: 75e6,
        ..BASE
    },
    Profile {
        id: "stereo",
        name: "STEREO space weather beacon",
        modulation: Modulation::Bpsk,
        symbol_rate: 1.44e6,
        roll_off: 0.35,
        conv: Conv::Turbo,
        rand: Rand::None,
        frame: 1119,
        rate: 3e6,
        least: 2e6,
        ..BASE
    },
    Profile {
        id: "oceansat2",
        name: "Oceansat-2 X band",
        symbol_rate: 21.226e6,
        diff: Diff::Dqpsk1,
        rand: Rand::None,
        frame: 0,
        header: Header::None,
        instrument: Instrument::Ocm,
        rate: 35e6,
        least: 28e6,
        ..BASE
    },
    Profile {
        id: "oceansat3_l",
        name: "Oceansat-3 L band",
        symbol_rate: 1e6,
        diff: Diff::NrzM,
        conv: Conv::Ccsds,
        rs: rs(16, 4, 0),
        rate: 3e6,
        least: 1.5e6,
        ..BASE
    },
    Profile {
        id: "saral_l",
        name: "SARAL L band",
        symbol_rate: 400e3,
        diff: Diff::NrzM,
        conv: Conv::Ccsds,
        rs: rs(16, 4, 0),
        rate: 3e6,
        least: 0.6e6,
        ..BASE
    },
    Profile {
        id: "oceansat3_x",
        name: "Oceansat-3 X band",
        symbol_rate: 80e6,
        roll_off: 0.35,
        rs: rs(8, 8, 0),
        frame: 2044,
        rate: 120e6,
        least: 90e6,
        ..BASE
    },
    Profile {
        id: "saral_x",
        name: "SARAL X band",
        symbol_rate: 16e6,
        conv: Conv::Ccsds,
        rs: rs(16, 5, 0),
        frame: 1279,
        rate: 25e6,
        least: 20e6,
        ..BASE
    },
    Profile {
        id: "kanopus_v",
        name: "Kanopus-V X band",
        symbol_rate: 61.44e6,
        roll_off: 0.25,
        diff: Diff::FourPhase,
        rand: Rand::None,
        frame: 0,
        header: Header::None,
        instrument: Instrument::Kanopus,
        rate: 90e6,
        least: 72e6,
        ..BASE
    },
    Profile {
        id: "dmsp_rtd",
        name: "DMSP real-time data",
        modulation: Modulation::Bpsk,
        symbol_rate: 1.024e6,
        rand: Rand::None,
        frame: 0,
        header: Header::None,
        instrument: Instrument::Ols,
        rate: 2e6,
        least: 1.5e6,
        ..BASE
    },
];

/// The profile of a submode id; the first profile for `None` or an unknown id.
pub fn profile(id: Option<&str>) -> &'static Profile {
    id.and_then(|id| PROFILES.iter().find(|p| p.id == id))
        .unwrap_or(&PROFILES[0])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derived_sizes_follow_the_table() {
        let p = profile(Some("jpss_hrd"));
        assert_eq!((p.data_bytes(), p.zone_at() + p.zone), (892, 892));
        let p = profile(Some("metop_dump"));
        assert_eq!((p.data_bytes(), p.zone_at() + p.zone), (892, 892));
        let p = profile(Some("jpss_hrd_n21"));
        assert_eq!(p.data_bytes(), 1115);
        assert!(p.zone_at() + p.zone <= p.data_bytes());
        let p = profile(Some("gcom_w1"));
        assert_eq!((1264 - 4) / 5, 255 - 3);
        assert!(p.zone_at() + p.zone <= p.data_bytes());
        let p = profile(Some("proba_s"));
        assert!(p.zone_at() + p.zone <= p.data_bytes());
        assert_eq!(profile(Some("oceansat3_x")).data_bytes(), 2044 - 4 - 16 * 8);
        assert_eq!((2044 - 4) / 8, 255);
        assert_eq!(PROFILES.iter().filter(|p| p.has_cadus()).count(), 17);
    }
}
