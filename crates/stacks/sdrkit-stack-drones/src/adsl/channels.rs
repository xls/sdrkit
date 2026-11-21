//! The channels of `adsl`: the four ADS-L channel receivers (M-band 868.2 and 868.4 MHz, O-band LDR and HDR on 869.525 MHz),
//! each a configuration of the chip receiver of the crate, the reader of the packet after a sync with its parity and repair, the
//! two FLARM channels of the same capture, and the frequency settings of the channels.

use super::proto::{self, crc24_ok, crc24_syndrome, NetHeader, LDR_HEADER, TRAFFIC_PACKET};
use crate::chips::{ChipChannel, ChipConfig, ChipStats, ChipView, Detector, Outcome, Reader};
use crate::flarm::demod::{Channel, ChannelItem, FlarmChannel, FlarmFrame};
use sdrkit_algo_fec::rs::Rs;
use sdrkit_core::command::{CommandError, CommandId, CommandSet, Commands};
use sdrkit_core::publish::field;
use sdrkit_core::{
    Choice, ConfigError, Record, Setting, SettingError, SettingId, SettingSet, Value, Values,
};
use sdrkit_graph::{
    values_of, Applied, Component, ComponentInfo, Format, Io, Item, Kind, Port, RateFrom, TagKind,
    Timed,
};
use std::sync::OnceLock;

/// The centre of a capture that holds all four channels and FLARM at 2 Msps, Hz.
pub const WIDE_CENTRE_HZ: f64 = 868.875e6;
/// The centre of a capture that holds the M-band only, Hz.
pub const NARROW_CENTRE_HZ: f64 = 868.3e6;
/// A capture whose centre is not known is taken as the wide one from this rate on.
pub const WIDE_RATE_HZ: f64 = 1.6e6;

sdrkit_core::choice_enum! {
    /// The four channels of ADS-L.
    #[derive(Default, serde::Serialize, serde::Deserialize)]
    pub enum Band {
        /// M-band 1: 868.2 MHz, Manchester at 100 kchip/s.
        #[default]
        M1 = ("m1", "868.2 MHz"),
        /// M-band 2: 868.4 MHz, Manchester at 100 kchip/s.
        M2 = ("m2", "868.4 MHz"),
        /// O-band low data rate: 869.525 MHz, 38.4 kchip/s.
        Ldr = ("ldr", "869.525 MHz LDR"),
        /// O-band high data rate: 869.525 MHz, 200 kchip/s.
        Hdr = ("hdr", "869.525 MHz HDR"),
    }
}

/// The four bands in the order of their counters.
pub const BANDS: [Band; 4] = [Band::M1, Band::M2, Band::Ldr, Band::Hdr];

impl Band {
    /// The place of the band in [`BANDS`].
    pub fn slot(self) -> usize {
        usize::from(self.index())
    }
    /// The frequency of the band in the test signal, Hz: the default of its frequency setting.
    pub fn frequency_hz(self) -> f64 {
        match self {
            Band::M1 => 868.2e6,
            Band::M2 => 868.4e6,
            Band::Ldr | Band::Hdr => 869.525e6,
        }
    }
    /// The pass edge of the channel filter, Hz.
    pub fn pass_hz(self) -> f64 {
        match self {
            Band::M1 | Band::M2 => 85e3,
            Band::Ldr => 32e3,
            Band::Hdr => 130e3,
        }
    }
    /// The band has a length byte in front of the network packet.
    pub fn has_length(self) -> bool {
        self != Band::Ldr
    }
    /// The signal of the band. `tone` selects the tone-energy detector for the M-band.
    pub fn chip_config(self, tone: bool) -> ChipConfig {
        match self {
            Band::M1 | Band::M2 => ChipConfig {
                chip_rate: 100e3,
                samples_per_chip: 8,
                deviation_hz: 50e3,
                pass_hz: 85e3,
                stop_hz: 140e3,
                detector: if tone {
                    Detector::ToneEnergy
                } else {
                    Detector::Discriminator
                },
                manchester: true,
                // Four preamble ones, the 0101 of the preamble end, and the sync word 72 4B.
                sync: 0xF5_724B,
                sync_bits: 24,
                skip_samples: 400,
                ring_chips: 8192,
            },
            Band::Ldr => ChipConfig {
                chip_rate: 38.4e3,
                samples_per_chip: 8,
                deviation_hz: 12.5e3,
                pass_hz: 32e3,
                stop_hz: 60e3,
                detector: Detector::Discriminator,
                manchester: false,
                // Eight preamble chips and the sync word B4 2B.
                sync: 0xAA_B42B,
                sync_bits: 24,
                skip_samples: 400,
                ring_chips: 1024,
            },
            Band::Hdr => ChipConfig {
                chip_rate: 200e3,
                samples_per_chip: 4,
                deviation_hz: 50e3,
                pass_hz: 130e3,
                stop_hz: 210e3,
                detector: Detector::Discriminator,
                manchester: false,
                // Eight preamble chips and the sync word 2D D4.
                sync: 0xAA_2DD4,
                sync_bits: 24,
                skip_samples: 400,
                ring_chips: 4096,
            },
        }
    }
    /// Whether the band at `freq_hz` lies inside a capture at `rate` centred on `centre_hz`: its offset and its pass edge stay
    /// below 0.49 of the rate.
    pub fn fits(self, freq_hz: f64, rate: f64, centre_hz: f64) -> bool {
        (freq_hz - centre_hz).abs() + self.pass_hz() < 0.49 * rate
    }
}

/// The centre of a capture at `rate`: the stated centre, else 868.875 MHz for a wide capture and 868.3 MHz for a narrow one.
pub fn capture_centre(rate: f64, stated_hz: f64) -> f64 {
    if stated_hz > 0.0 {
        stated_hz
    } else if rate >= WIDE_RATE_HZ {
        WIDE_CENTRE_HZ
    } else {
        NARROW_CENTRE_HZ
    }
}

/// A network packet with good parity.
#[derive(Clone, Debug, PartialEq)]
pub struct AdslPacket {
    /// The network header.
    pub head: NetHeader,
    /// The ADS-L data as sent (scrambled unless the key index is 3), without the parity.
    pub data: Vec<u8>,
    /// The band.
    pub band: Band,
    /// The frequency of the band, Hz.
    pub freq_hz: f64,
    /// The level over the noise floor of the channel, dB.
    pub level_db: f64,
    /// The packet needed a repair: bits of a Traffic packet, or bytes by the Reed-Solomon code.
    pub repaired: bool,
}

/// The state of one band, at each report.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct BandState {
    /// The band.
    pub band: Band,
    /// The band is inside the capture and is received.
    pub on: bool,
    /// The counters.
    pub stats: ChipStats,
    /// The noise floor, dBFS for each sample.
    pub noise_dbfs: Option<f64>,
    /// The time of the last block with energy, seconds since the configure.
    pub energy_at: Option<f64>,
}

/// What the channels report to the tracker.
#[derive(Clone, Debug, PartialEq)]
pub enum AdslItem {
    /// A packet with good parity.
    Packet(AdslPacket),
    /// The state of a band, at each report.
    Band(BandState),
    /// A FLARM frame of the same capture, or the counters of a FLARM channel.
    Flarm(ChannelItem),
}

impl Item for AdslItem {
    const KIND: &'static str = "adsl.item";
}

/// The syndromes of each single wrong bit of a Traffic packet, in the order the bits are sent.
fn bit_syndromes() -> &'static [u32; TRAFFIC_PACKET * 8] {
    static TABLE: OnceLock<[u32; TRAFFIC_PACKET * 8]> = OnceLock::new();
    TABLE.get_or_init(|| {
        std::array::from_fn(|i| {
            let mut p = [0u8; TRAFFIC_PACKET];
            p[i / 8] = 0x80 >> (i % 8);
            crc24_syndrome(&p)
        })
    })
}

/// Repairs a Traffic packet whose CRC fails: every subset of the 4 bits with the smallest margins is flipped in turn, and when
/// the syndrome that is left is the syndrome of one single bit, that bit is flipped too. Returns whether a case gave a good CRC.
pub fn repair(packet: &mut [u8; TRAFFIC_PACKET], margins: &[f32]) -> bool {
    let syn = bit_syndromes();
    let mut weak = [0usize; 4];
    let mut order: [usize; TRAFFIC_PACKET * 8] = std::array::from_fn(|i| i);
    for slot in 0..4 {
        let mut best = slot;
        for j in slot + 1..order.len() {
            if margins.get(order[j]).copied().unwrap_or(f32::MAX)
                < margins.get(order[best]).copied().unwrap_or(f32::MAX)
            {
                best = j;
            }
        }
        order.swap(slot, best);
        weak[slot] = order[slot];
    }
    let base = crc24_syndrome(packet);
    for case in 0u32..16 {
        let mut s = base;
        for (b, &i) in weak.iter().enumerate() {
            if case >> b & 1 != 0 {
                s ^= syn[i];
            }
        }
        let extra = if s == 0 {
            None
        } else {
            match syn.iter().position(|&x| x == s) {
                Some(i) => Some(i),
                None => continue,
            }
        };
        for (b, &i) in weak.iter().enumerate() {
            if case >> b & 1 != 0 {
                packet[i / 8] ^= 0x80 >> (i % 8);
            }
        }
        if let Some(i) = extra {
            packet[i / 8] ^= 0x80 >> (i % 8);
        }
        return true;
    }
    false
}

/// Reads the packet after a sync: the length byte or the fixed header, the parity, and the repair of a Traffic packet.
pub struct AdslReader {
    band: Band,
    freq_hz: f64,
    packets: Vec<(f64, AdslPacket)>,
    rs: Vec<(usize, Rs)>,
    bytes: Vec<u8>,
    margins: Vec<f32>,
}

impl AdslReader {
    /// The reader of `band`.
    pub fn new(band: Band) -> Self {
        AdslReader {
            band,
            freq_hz: band.frequency_hz(),
            packets: Vec::new(),
            rs: Vec::new(),
            bytes: Vec::with_capacity(256),
            margins: Vec::with_capacity(TRAFFIC_PACKET * 8),
        }
    }

    /// Checks the parity of the `len` bytes that start at byte `at` after the sync. Returns the packet without its parity, and
    /// whether it needed a repair.
    fn packet(&mut self, view: &ChipView<'_>, centre: f32, at: usize, len: usize) -> Option<bool> {
        self.bytes.clear();
        self.bytes
            .extend((0..len).map(|j| view.byte(at + j, centre)));
        if crc24_ok(&self.bytes) {
            self.bytes.truncate(len - 3);
            return Some(false);
        }
        if len > proto::RS_PARITY {
            if !self.rs.iter().any(|(n, _)| *n == len) {
                self.rs.push((len, proto::rs_for(len)?));
            }
            let rs = &mut self.rs.iter_mut().find(|(n, _)| *n == len)?.1;
            let fixed = rs.decode(&mut self.bytes, &[]).ok()?;
            self.bytes.truncate(len - proto::RS_PARITY);
            return Some(fixed.corrected > 0);
        }
        if len == TRAFFIC_PACKET {
            self.margins.clear();
            self.margins
                .extend((0..len * 8).map(|j| view.bit(at * 8 + j, centre).1));
            let mut p = [0u8; TRAFFIC_PACKET];
            p.copy_from_slice(&self.bytes);
            if repair(&mut p, &self.margins) {
                self.bytes.clear();
                self.bytes.extend_from_slice(&p[..len - 3]);
                return Some(true);
            }
        }
        None
    }
}

impl Reader for AdslReader {
    fn first_need(&self) -> u64 {
        let cfg = self.band.chip_config(true);
        let bytes = if self.band.has_length() {
            1
        } else {
            LDR_HEADER.len() + TRAFFIC_PACKET
        };
        bytes as u64 * 8 * cfg.chips_per_bit()
    }

    fn read(&mut self, view: &ChipView<'_>) -> Outcome {
        let centre = view.centre();
        let per_byte = 8 * view.chips_per_bit();
        let (at, lengths) = if self.band.has_length() {
            let len = usize::from(view.byte(0, centre));
            let want = (1 + len.max(TRAFFIC_PACKET)) as u64 * per_byte;
            if view.chips_after() < want {
                return Outcome::Need(want);
            }
            // The length byte has no protection: after it, the lengths of a Traffic and of a Status packet are tried.
            let first = if len >= 8 { len } else { TRAFFIC_PACKET };
            (1, [first, TRAFFIC_PACKET, proto::STATUS_PACKET])
        } else {
            let wrong: u32 = LDR_HEADER
                .iter()
                .enumerate()
                .map(|(j, h)| (view.byte(j, centre) ^ h).count_ones())
                .sum();
            if wrong > 2 {
                return Outcome::Bad;
            }
            (LDR_HEADER.len(), [TRAFFIC_PACKET; 3])
        };
        for (i, &len) in lengths.iter().enumerate() {
            if lengths[..i].contains(&len) {
                continue;
            }
            let Some(repaired) = self.packet(view, centre, at, len) else {
                continue;
            };
            let head = NetHeader::of(self.bytes[0]);
            let level_db = view.level_db(-15, (at + len.min(32)) as i64 * per_byte as i64);
            self.packets.push((
                view.sync_time_s(),
                AdslPacket {
                    head,
                    data: self.bytes[1..].to_vec(),
                    band: self.band,
                    freq_hz: self.freq_hz,
                    level_db,
                    repaired,
                },
            ));
            return Outcome::Good;
        }
        Outcome::Bad
    }
}

/// The frequency of M-band 1. The default is the one of the test signal.
pub const M1_HZ: Setting<field::Hz> = Setting::new("m1_hz", "M-band 1", "Hz", 868.2e6)
    .range(100e6, 2e9, 1e3)
    .help("The first Manchester channel. The default is the frequency of the test signal.");
/// The frequency of M-band 2. The default is the one of the test signal.
pub const M2_HZ: Setting<field::Hz> = Setting::new("m2_hz", "M-band 2", "Hz", 868.4e6)
    .range(100e6, 2e9, 1e3)
    .help("The second Manchester channel. The default is the frequency of the test signal.");
/// The frequency of the O-band (LDR and HDR). The default is the one of the test signal.
pub const O_HZ: Setting<field::Hz> = Setting::new("o_hz", "O-band", "Hz", 869.525e6)
    .range(100e6, 2e9, 1e3)
    .help("The channel of the low and the high data rate. The default is the frequency of the test signal.");
/// Decode FLARM from the same capture.
pub const WITH_FLARM: Setting<field::Bool> =
    Setting::new("with_flarm", "Decode FLARM from the same capture", "", true);
/// The M-band chip detector by tone energy.
pub const TONE_CHIPS: Setting<field::Bool> = Setting::new(
    "tone_chips",
    "M-band chip detector by tone energy",
    "",
    true,
)
.help("Off selects the FM discriminator, for comparisons.")
.advanced();

sdrkit_core::settings! {
    /// The settings of `adsl.channels`.
    pub enum AdslChannelsSetting {
        /// M-band 1.
        M1Hz = M1_HZ,
        /// M-band 2.
        M2Hz = M2_HZ,
        /// O-band.
        OHz = O_HZ,
        /// Decode FLARM from the same capture.
        WithFlarm = WITH_FLARM,
        /// M-band chip detector by tone energy.
        ToneChips = TONE_CHIPS,
    }
}

/// The channels component (`adsl.channels`): IQ in, packets, FLARM frames and the state of each band out.
pub struct AdslChannels {
    bands: [ChipChannel<AdslReader>; 4],
    flarm: [FlarmChannel; 2],
    flarm_frames: Vec<(f64, FlarmFrame)>,
    freqs: [f64; 4],
    with_flarm: bool,
    tone: bool,
    rate: f64,
    centre_hz: f64,
}

impl Default for AdslChannels {
    fn default() -> Self {
        AdslChannels {
            bands: BANDS.map(|b| ChipChannel::new(b.chip_config(true), AdslReader::new(b))),
            flarm: [
                FlarmChannel::new(Channel::Low),
                FlarmChannel::new(Channel::High),
            ],
            flarm_frames: Vec::new(),
            freqs: BANDS.map(Band::frequency_hz),
            with_flarm: true,
            tone: true,
            rate: 0.0,
            centre_hz: 0.0,
        }
    }
}

impl AdslChannels {
    /// Starts or stops each channel for the capture and the settings.
    fn start(&mut self) -> Result<(), ConfigError> {
        if self.rate <= 0.0 {
            return Ok(());
        }
        let centre = capture_centre(self.rate, self.centre_hz);
        for ((b, f), ch) in BANDS.iter().zip(self.freqs).zip(&mut self.bands) {
            ch.reader.freq_hz = f;
            if b.fits(f, self.rate, centre) {
                ch.configure(self.rate, f - centre)?;
            } else {
                ch.stop();
            }
        }
        for (c, ch) in [Channel::Low, Channel::High].iter().zip(&mut self.flarm) {
            let fits = (c.frequency_hz() - centre).abs() + 85e3 < 0.49 * self.rate;
            if self.with_flarm && fits {
                ch.configure(self.rate, NARROW_CENTRE_HZ - centre)?;
            } else {
                ch.stop();
            }
        }
        Ok(())
    }
}

impl Component for AdslChannels {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "adsl.channels",
            vec![Port::iq("in")],
            vec![Port::typed::<AdslItem>("out").rate_from(RateFrom::Fixed(0.0))],
        )
        .with_settings(AdslChannelsSetting::schema())
    }

    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let f = &inputs[0];
        self.rate = f.rate;
        self.centre_hz = f.centre_hz;
        self.start()?;
        Ok(vec![Format::new(Kind::typed::<AdslItem>(), 0.0)])
    }

    fn process(&mut self, io: &mut Io) {
        let retuned = io.input(0).tags().iter().rev().find_map(|t| match t.kind {
            TagKind::Retuned { centre_hz, .. } => Some(centre_hz),
            _ => None,
        });
        if let Some(c) = retuned {
            self.centre_hz = c;
            let _ = self.start();
        }
        let x = io.iq(0);
        for ch in &mut self.bands {
            ch.feed(x);
        }
        self.flarm_frames.clear();
        for ch in &mut self.flarm {
            ch.feed(x, &mut self.flarm_frames);
        }
        let tick = io.tick();
        let o = io.items_out::<AdslItem>(0);
        for ch in &mut self.bands {
            for (t, p) in ch.reader.packets.drain(..) {
                o.push(Timed {
                    time: t,
                    value: AdslItem::Packet(p),
                });
            }
        }
        for (t, f) in self.flarm_frames.drain(..) {
            o.push(Timed {
                time: t,
                value: AdslItem::Flarm(ChannelItem::Frame(f)),
            });
        }
        if tick {
            for (b, ch) in BANDS.iter().zip(&self.bands) {
                o.push(Timed {
                    time: 0.0,
                    value: AdslItem::Band(BandState {
                        band: *b,
                        on: ch.running(),
                        stats: ch.stats(),
                        noise_dbfs: ch.noise_dbfs(),
                        energy_at: ch.energy_at(),
                    }),
                });
            }
            for ch in &self.flarm {
                o.push(Timed {
                    time: 0.0,
                    value: AdslItem::Flarm(ChannelItem::Stats(ch.stats())),
                });
            }
        }
    }

    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match AdslChannelsSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            AdslChannelsSetting::M1Hz => self.freqs[0] = M1_HZ.read(id, v)?,
            AdslChannelsSetting::M2Hz => self.freqs[1] = M2_HZ.read(id, v)?,
            AdslChannelsSetting::OHz => {
                let f = O_HZ.read(id, v)?;
                self.freqs[2] = f;
                self.freqs[3] = f;
            }
            AdslChannelsSetting::WithFlarm => self.with_flarm = WITH_FLARM.read(id, v)?,
            AdslChannelsSetting::ToneChips => {
                let tone = TONE_CHIPS.read(id, v)?;
                if tone != self.tone {
                    self.tone = tone;
                    for b in [Band::M1, Band::M2] {
                        self.bands[b.slot()] =
                            ChipChannel::new(b.chip_config(tone), AdslReader::new(b));
                    }
                }
            }
        }
        Ok(Applied::Reformat)
    }

    fn values(&self) -> Values {
        values_of([
            (AdslChannelsSetting::M1Hz, Value::Hz(self.freqs[0])),
            (AdslChannelsSetting::M2Hz, Value::Hz(self.freqs[1])),
            (AdslChannelsSetting::OHz, Value::Hz(self.freqs[2])),
            (AdslChannelsSetting::WithFlarm, Value::Bool(self.with_flarm)),
            (AdslChannelsSetting::ToneChips, Value::Bool(self.tone)),
        ])
    }

    fn commands(&self) -> &Commands {
        AdslChannelsCommand::commands()
    }

    fn invoke(&mut self, id: CommandId, _args: &Values) -> Result<Option<Record>, CommandError> {
        match AdslChannelsCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            AdslChannelsCommand::Clear => {
                for ch in &mut self.bands {
                    ch.reset_stats();
                }
            }
        }
        Ok(None)
    }
}

sdrkit_core::commands! {
    /// The commands of `adsl.channels`.
    pub enum AdslChannelsCommand {
        /// Sets the counters of the channels to zero.
        Clear = sdrkit_util_objects::commands::CLEAR,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_band_is_received_when_it_fits_the_capture() {
        let on = |rate: f64, centre: f64| BANDS.map(|b| b.fits(b.frequency_hz(), rate, centre));
        for rate in [1.0e6, 1.024e6, 1.2e6] {
            assert_eq!(on(rate, 868.3e6), [true, true, false, false], "{rate}");
        }
        assert_eq!(on(1e6, 869.525e6), [false, false, true, true]);
        assert_eq!(on(2e6, 868.875e6), [true; 4]);
        assert_eq!(capture_centre(2e6, 0.0), 868.875e6);
        assert_eq!(capture_centre(1.2e6, 0.0), 868.3e6);
        assert_eq!(capture_centre(1.2e6, 869.525e6), 869.525e6);
    }

    #[test]
    fn a_traffic_packet_with_wrong_bits_is_repaired() {
        let good: [u8; 24] = [
            0x01, 0x57, 0xA8, 0xE2, 0x6B, 0x5A, 0x73, 0xBA, 0x8D, 0x52, 0x77, 0xB4, 0x7E, 0x4B,
            0x6D, 0xD6, 0xEF, 0xF3, 0x5E, 0xDC, 0xDA, 0xF0, 0x65, 0xE0,
        ];
        let mut margins = vec![1.0f32; 192];
        // One wrong bit anywhere, with a large margin.
        let mut p = good;
        p[9] ^= 0x10;
        assert!(repair(&mut p, &margins));
        assert_eq!(p, good);
        // Four wrong bits with the smallest margins, and a fifth elsewhere.
        let mut p = good;
        for (k, i) in [3usize, 50, 97, 150].iter().enumerate() {
            p[i / 8] ^= 0x80 >> (i % 8);
            margins[*i] = 0.01 * (k + 1) as f32;
        }
        p[20] ^= 0x01;
        assert!(repair(&mut p, &margins));
        assert_eq!(p, good);
        // Two wrong bits with large margins are not found.
        let mut p = good;
        p[1] ^= 0x01;
        p[14] ^= 0x40;
        assert!(!repair(&mut p, &[1.0; 192]) || p != good);
    }
}
