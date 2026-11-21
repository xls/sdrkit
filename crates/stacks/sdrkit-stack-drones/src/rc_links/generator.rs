//! The `rc_links` test signal at unit power less 3 dB: GFSK bursts (BT 0.5, a 1 the higher frequency) of a FrSky D8 and a
//! D16 transmitter, an AFHDS transmitter sent least significant bit first and an AFHDS 2A transmitter sent most significant
//! bit first at the same time, a Futaba S-FHSS transmitter sending pairs of packets, and DSMX and DSM2 transmitters on 1 MHz
//! channels of the band (the radio centre taken as [`TEST_CENTRE_HZ`]), over a background floor of -30 dBFS. The places of
//! the FSK links scale with the sample rate; a link whose least rate is above the sample rate is left out. Every 50 ms each
//! transmitter sends.

use super::{dsm, flysky, frsky, sfhss, Link};
use crate::fsklink::bits_of;
use crate::mavlink_sik::generator::gfsk;
use sdrkit_algo_sim::scene::{Burst, Scene};
use sdrkit_core::{Complex32, ConfigError, GenConfig, Generator, SignalInfo};

/// The radio centre that the test signal assumes: the default frequency of the stack, Hz.
pub const TEST_CENTRE_HZ: f64 = 2440e6;
/// The cycle, s.
pub const PERIOD_S: f64 = 0.05;
/// The FrSky D8 id.
pub const D8_ID: u16 = 0xABCD;
/// The FrSky D8 channels.
pub const D8_CHANNELS: [u16; 8] = [1500, 1600, 1700, 1800, 2000, 2200, 2400, 2600];
/// The FrSky D16 id.
pub const D16_ID: u16 = 0x5A3C;
/// The AFHDS id (shown p4 p3 p2 p1).
pub const AFHDS_ID: u32 = 0x1234_5678;
/// The AFHDS 2A id (shown p1 p2 p3 p4).
pub const AFHDS2A_ID: u32 = 0x8765_4321;
/// The AFHDS 2A receiver id.
pub const AFHDS2A_RX: u32 = 0x0BAD_F00D;
/// The S-FHSS id.
pub const SFHSS_ID: u16 = 0xBC11;
/// The DSMX transmitter id bytes.
pub const DSMX_ID: [u8; 4] = [0x12, 0x34, 0x56, 0x79];
/// The DSM2 transmitter id bytes.
pub const DSM2_ID: [u8; 4] = [0x0A, 0x1B, 0x2C, 0x3D];
/// The DSM2 rf channel.
pub const DSM2_RF: u8 = 41;

/// One transmitter of the test signal.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Tx {
    /// FrSky D8.
    D8,
    /// FrSky D16 at 99 970 Bd.
    D16,
    /// AFHDS.
    Afhds,
    /// AFHDS 2A.
    Afhds2a,
    /// S-FHSS.
    Sfhss,
    /// DSMX.
    Dsmx,
    /// DSM2.
    Dsm2,
}

impl Tx {
    /// Every transmitter.
    pub const ALL: [Tx; 7] = [Tx::D8, Tx::D16, Tx::Afhds, Tx::Afhds2a, Tx::Sfhss, Tx::Dsmx, Tx::Dsm2];

    /// Its link.
    pub fn link(self) -> Link {
        match self {
            Tx::D8 | Tx::D16 => Link::Frsky,
            Tx::Afhds | Tx::Afhds2a => Link::Flysky,
            Tx::Sfhss => Link::Sfhss,
            Tx::Dsmx | Tx::Dsm2 => Link::Dsm,
        }
    }

    /// When it sends in the cycle, s.
    fn at_s(self) -> f64 {
        match self {
            Tx::D8 => 0.002,
            Tx::D16 => 0.010,
            Tx::Afhds => 0.015,
            Tx::Afhds2a => 0.0152,
            Tx::Sfhss => 0.019,
            Tx::Dsmx => 0.024,
            Tx::Dsm2 => 0.030,
        }
    }

    /// Its place as a fraction of the sample rate (the FSK links).
    fn place(self) -> f64 {
        match self {
            Tx::D8 => -0.30,
            Tx::D16 => -0.18,
            Tx::Afhds => 0.08,
            Tx::Afhds2a => 0.30,
            Tx::Sfhss => -0.06,
            Tx::Dsmx | Tx::Dsm2 => 0.0,
        }
    }
}

/// One packet of an FSK transmitter.
#[derive(Clone, Debug, PartialEq)]
pub struct FskPacket {
    /// The bytes after the pattern.
    pub bytes: Vec<u8>,
    /// The air pattern of the link (two preamble bytes and the sync word or ID code).
    pub pattern: &'static [u8],
    /// The bit rate, bit/s.
    pub rate: f64,
    /// The deviation, Hz.
    pub deviation_hz: f64,
    /// Bytes go least significant bit first.
    pub lsb_first: bool,
}

impl FskPacket {
    /// The air bits: 4 bytes of AA, the rest of the pattern after its first two bytes, the bytes and a tail byte sent while
    /// the transmitter ramps down, each byte in the bit order of the link.
    pub fn air_bits(&self) -> Vec<u8> {
        let mut all = vec![0xAA; 4];
        all.extend_from_slice(&self.pattern[2..]);
        all.extend_from_slice(&self.bytes);
        all.push(crate::mavlink_sik::generator::TAIL);
        let mut bits = Vec::with_capacity(8 * all.len());
        bits_of(&all, self.lsb_first, &mut bits);
        bits
    }
}

/// Packet `n` of an FSK transmitter; `None` for a DSM transmitter.
pub fn fsk_packet(tx: Tx, n: u32) -> Option<FskPacket> {
    let p = |bytes: Vec<u8>, pattern: &'static [u8], rate: f64, deviation_hz: f64, lsb_first: bool| FskPacket {
        bytes,
        pattern,
        rate,
        deviation_hz,
        lsb_first,
    };
    Some(match tx {
        Tx::D8 => p(
            frsky::d8_channels(D8_ID, n as u8, &D8_CHANNELS).to_vec(),
            &frsky::PATTERN,
            frsky::D8_RATE,
            31.7e3,
            false,
        ),
        Tx::D16 => p(
            frsky::d16(D16_ID, (n % 47) as u8, 25, false, 29, 0x55),
            &frsky::PATTERN,
            frsky::D16_RATES[1],
            57e3,
            false,
        ),
        Tx::Afhds => {
            let ch = std::array::from_fn(|k| 1000 + 100 * k as u16 + (n % 10) as u16);
            p(flysky::afhds(AFHDS_ID, &ch).to_vec(), &flysky::PATTERN, flysky::RATE, 186e3, true)
        }
        Tx::Afhds2a => {
            let ch = std::array::from_fn(|k| 1100 + 50 * k as u16);
            p(
                flysky::afhds2a(AFHDS2A_ID, AFHDS2A_RX, &ch).to_vec(),
                &flysky::PATTERN,
                flysky::RATE,
                186e3,
                false,
            )
        }
        Tx::Sfhss => {
            let s = sfhss::Sfhss {
                head: 0x81,
                id: SFHSS_ID,
                hop: (7 + n) as u8 % sfhss::HOP_CHANNELS,
                code: 11,
                command: if n % 2 == 1 { 0x09 } else { 0x02 },
                channels: [1520, 1020, 2020, 1777],
                crc: sfhss::SfhssCrc::Ccitt,
            };
            p(sfhss::build(&s).to_vec(), &sfhss::PATTERN, sfhss::RATE, 38_086.0, false)
        }
        Tx::Dsmx | Tx::Dsm2 => return None,
    })
}

/// The rf channel and the chips of DSM packet `n` of `tx`.
pub fn dsm_packet(tx: Tx, n: u32, fs: f64) -> (u8, Vec<u8>) {
    let (dsmx, id) = match tx {
        Tx::Dsmx => (true, DSMX_ID),
        _ => (false, DSM2_ID),
    };
    // The rf channels within the capture, 1 MHz from its edges.
    let half = ((fs / 2.0 - 1e6) / 1e6).floor().max(0.0) as i32;
    let centre = ((TEST_CENTRE_HZ - 2400e6) / 1e6).round() as i32;
    let inside = |rf: u8| (i32::from(rf) - centre).abs() <= half;
    let rf = if dsmx {
        let list: Vec<u8> = dsm::dsmx_channels(id)
            .into_iter()
            .filter(|&c| inside(c))
            .collect();
        if list.is_empty() {
            centre as u8
        } else {
            list[n as usize % list.len()]
        }
    } else {
        DSM2_RF
    };
    let set = dsm::CodeSet::new(dsm::pn_row(rf, dsmx), dsm::sop_column(id));
    let head = if dsmx {
        [id[2], id[3]]
    } else {
        [!id[2], !id[3]]
    };
    let second = n % 2 == 1;
    let ch: Vec<(u8, u16)> = (0..7u8)
        .map(|i| (i + if second { 7 } else { 0 }, 400 + 150 * u16::from(i)))
        .filter(|&(i, _)| i <= 12)
        .collect();
    let bits = if dsmx { 11 } else { 10 };
    let d = dsm::data(head, bits, &ch, second);
    let mut chips = Vec::with_capacity(dsm::PACKET_CHIPS);
    dsm::packet_chips(&set, &d, [0x5A, 0xA5], &mut chips);
    // Chips while the transmitter ramps down.
    chips.extend([0, 1, 0, 1, 0, 1, 0, 1]);
    (rf, chips)
}

/// The test signal at `fs` with the transmitters of `links`.
///
/// # Errors
///
/// [`ConfigError`] if the scene refuses the rate.
pub fn scene(fs: f64, links: &[Link]) -> Result<Scene, ConfigError> {
    let mut scene = Scene::new(fs)?;
    crate::mavlink_sik::generator::background(&mut scene, fs, 0x2C);
    for tx in Tx::ALL {
        if !links.contains(&tx.link()) || fs < tx.link().least_rate_hz() {
            continue;
        }
        let mut n = 0u32;
        scene.add_source(Box::new(move || {
            let start_s = tx.at_s() + f64::from(n) * PERIOD_S;
            let (samples, offset_hz) = match fsk_packet(tx, n) {
                Some(p) => (
                    gfsk(&p.air_bits(), p.rate, p.deviation_hz, fs).ok()?,
                    tx.place() * fs,
                ),
                None => {
                    let (rf, chips) = dsm_packet(tx, n, fs);
                    (
                        gfsk(&chips, dsm::CHIP_RATE, 250e3, fs).ok()?,
                        2400e6 + f64::from(rf) * 1e6 - TEST_CENTRE_HZ + 15e3,
                    )
                }
            };
            n += 1;
            Some(Burst {
                start_s,
                offset_hz,
                level_db: crate::mavlink_sik::generator::LEVEL_DB,
                samples,
            })
        }));
    }
    Ok(scene)
}

/// The `rc_links` generator.
pub struct RcGenerator {
    scene: Scene,
    rate: f64,
}

/// The generator of the settings of `cfg`: the links of the `submode` setting.
///
/// # Errors
///
/// As [`scene`].
pub fn from_config(cfg: &GenConfig) -> Result<RcGenerator, ConfigError> {
    let id = cfg.settings.schema().id(super::SUBMODE.key()).ok();
    let sub = id
        .and_then(|i| cfg.settings.value(i).and_then(|v| super::SUBMODE.read(i, v).ok()))
        .unwrap_or(super::Submode::All);
    Ok(RcGenerator {
        scene: scene(cfg.rate_hz, sub.links())?,
        rate: cfg.rate_hz,
    })
}

impl Generator for RcGenerator {
    fn rate_hz(&self) -> f64 {
        self.rate
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        self.scene.generate(out)
    }
    fn signal_info(&self) -> Option<SignalInfo> {
        Some(
            SignalInfo::new(500e3)
                .with_symbol_rate(100e3)
                .with_bit_rate(100e3)
                .with_power(1.0),
        )
    }
    fn station_offset_hz(&self, _tune_offset_hz: f64) -> f64 {
        0.0
    }
}
