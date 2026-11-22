//! 2.4 GHz radio-control links (`rc_links`): FrSky D8 and D16, FlySky AFHDS and AFHDS 2A, Futaba S-FHSS and Spektrum DSM2 and
//! DSMX, from bursts anywhere in the capture: the transmitter id and the channel values (FrSky D16: id and hop index). Hops are
//! not followed. No public document gives any of these links. The submode `ppm72` reads instead the PPM pulse trains of RC
//! model transmitters of 26 to 75 MHz ([`ppm`]).

pub mod dsm;
pub mod flysky;
pub mod frsky;
pub mod generator;
pub mod ppm;
pub mod sfhss;

use crate::family::{min_rate_message, wall, REPORT_S};
use crate::fsklink::{band_config, bytes_of, Burst, FskFront};
use crate::links::{
    LinkCommand,
    Heard, LinkHeld, NewLink, RcLinkSeen,
    RECENT_S,
};
use sdrkit_algo_modem::bandburst::BandDetector;
use sdrkit_algo_modem::burstmix::BurstMixer;
use sdrkit_algo_modem::fsksync::Polarity;
use crate::typed::{RcLinksPpmFields, FRAMES, LINKS, RC_LINKS_PPM_FRAMES};
use sdrkit_core::publish::{field, Published};
use sdrkit_core::command::{Command, CommandError, CommandId};
use sdrkit_core::value::Choice as _;
use sdrkit_core::Record;
use sdrkit_core::command::CommandSet as _;
use sdrkit_core::{
    AutoGain, Clock, Complex32, ConfigError, GenConfig, Generator, LockProfile, LockState,
    Receiver, ResultsDecl, Setting, SettingError, SettingId, SettingSet, StackConfig,
    StackDescriptor, StateBlob, StateError, Tuning, Value, Values,
};
use sdrkit_graph::{
    values_of, Catalog, Decoder, DecoderComponent, Format, GraphBuilder, GraphError, Kind, Outbox,
    Port, RunContext, StackGraph, Timed,
};
use sdrkit_util_objects::commands::Tables;
use sdrkit_util_objects::Stamp;
use sfhss::SfhssCrc;
use std::fmt::Write as _;
use std::sync::Arc;

/// The stack id.
pub const ID: &str = "rc_links";
/// The lowest input rate, Hz (each link has its own, [`Link::least_rate_hz`]).
pub const MIN_RATE_HZ: f64 = 1e6;
/// DSM bursts narrower than this are searched, Hz.
pub const DSM_MAX_WIDTH_HZ: f64 = 3.5e6;
/// The cut-off of the DSM channel filter, Hz.
pub const DSM_CUTOFF_HZ: f64 = 1.3e6;
/// The DSM channel needs at least this rate after decimation, Hz.
pub const DSM_MIN_WORK_HZ: f64 = 3.5e6;
/// A DSM packet with a larger residual carrier is refused, Hz.
pub const DSM_MAX_CFO_HZ: f64 = 600e3;
/// What no public document confirms.
pub const NOT_CONFIRMED: &str = "Not confirmed by a public document: no public document gives any layout of these links. The FlySky bit order (both are tried), which S-FHSS CRC is right (both are tried), the DSM chip polarity (setting `dsm_chip_polarity`, by default a 1 chip the higher frequency) and the DSM CRC (not checked), the frequency of an S-FHSS hop channel, and the FrSky D16 channel values (not unpacked). The submode ppm72 measures the PPM frame (channel windows, sync gap, pulse), which no standard or maker publishes; only AM keying is tested, not an FM-shifted carrier, and it needs an input of 1 Msps like the other submodes. Not tried on a real signal.";

/// A link of the stack.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Link {
    /// FrSky D8 and D16.
    Frsky,
    /// FlySky AFHDS and AFHDS 2A.
    Flysky,
    /// Futaba S-FHSS.
    Sfhss,
    /// Spektrum DSM2 and DSMX.
    Dsm,
}

impl Link {
    /// The name in the tables.
    pub fn name(self) -> &'static str {
        match self {
            Link::Frsky => "FrSky",
            Link::Flysky => "FlySky",
            Link::Sfhss => "Futaba S-FHSS",
            Link::Dsm => "Spektrum DSM",
        }
    }
    /// The least sample rate of the link, Hz.
    pub fn least_rate_hz(self) -> f64 {
        match self {
            Link::Frsky | Link::Sfhss => 1e6,
            Link::Flysky => 2.5e6,
            Link::Dsm => 4e6,
        }
    }
}

sdrkit_core::choice_enum! {
    /// The links searched.
    pub enum Submode {
        /// All four.
        All = ("all", "All"),
        /// FrSky D8 and D16.
        Frsky = ("frsky", "FrSky"),
        /// FlySky AFHDS and AFHDS 2A.
        Flysky = ("flysky", "FlySky"),
        /// Futaba S-FHSS.
        Sfhss = ("sfhss", "Futaba S-FHSS"),
        /// Spektrum DSM2 and DSMX.
        Dsm = ("dsm", "Spektrum DSM"),
        /// RC model PPM of 26 to 75 MHz; not part of All.
        Ppm72 = ("ppm72", "RC model PPM (26 to 75 MHz)"),
    }
}

impl Submode {
    /// The links of the choice.
    pub fn links(self) -> &'static [Link] {
        match self {
            Submode::All => &[Link::Frsky, Link::Sfhss, Link::Flysky, Link::Dsm],
            Submode::Frsky => &[Link::Frsky],
            Submode::Flysky => &[Link::Flysky],
            Submode::Sfhss => &[Link::Sfhss],
            Submode::Dsm => &[Link::Dsm],
            Submode::Ppm72 => &[],
        }
    }
}

sdrkit_core::choice_enum! {
    /// The FlySky bit order on the air.
    pub enum BitOrder {
        /// Both are tried.
        Both = ("both", "Both"),
        /// Most significant bit first.
        Msb = ("msb", "Most significant first"),
        /// Least significant bit first.
        Lsb = ("lsb", "Least significant first"),
    }
}

sdrkit_core::choice_enum! {
    /// The frequency of a DSM 1 chip.
    pub enum ChipPolarity {
        /// The higher frequency.
        High = ("high", "1 chip on the higher frequency"),
        /// The lower frequency.
        Low = ("low", "1 chip on the lower frequency"),
    }
}

/// The links searched.
pub const SUBMODE: Setting<field::Choice<Submode>> = Setting::choice("submode", "Link", Submode::All)
    .help("The links searched. All runs each link whose least sample rate the input reaches.");
/// The burst threshold.
pub const THRESHOLD_DB: Setting<field::F64> =
    Setting::new("threshold_db", "Burst threshold", "dB", 9.0)
        .range(6.0, 20.0, 1.0)
        .help("How far above the noise floor of a frequency bin a burst must rise.");
/// The FlySky bit order.
pub const FLYSKY_BIT_ORDER: Setting<field::Choice<BitOrder>> =
    Setting::choice("flysky_bit_order", "FlySky bit order", BitOrder::Both)
        .help("The bit order of FlySky bytes on the air. No public document gives it; both are tried by default.");
/// The DSM chip polarity.
pub const DSM_CHIP_POLARITY: Setting<field::Choice<ChipPolarity>> =
    Setting::choice("dsm_chip_polarity", "DSM chip polarity", ChipPolarity::High)
        .help("Which frequency a 1 chip of DSM is sent on. No public document gives it.");
/// The S-FHSS CRC.
pub const SFHSS_CRC: Setting<field::Choice<SfhssCrc>> =
    Setting::choice("sfhss_crc", "S-FHSS CRC", SfhssCrc::Either)
        .help("The CRC of S-FHSS packets. No public document gives it; both are tried by default and the one that matched is shown.");

sdrkit_core::settings! {
    /// The settings of `rc_links`.
    pub enum RcSetting {
        /// The links.
        Submode = SUBMODE,
        /// The burst threshold.
        ThresholdDb = THRESHOLD_DB,
        /// The FlySky bit order.
        FlyskyBitOrder = FLYSKY_BIT_ORDER,
        /// The DSM chip polarity.
        DsmChipPolarity = DSM_CHIP_POLARITY,
        /// The S-FHSS CRC.
        SfhssCrc = SFHSS_CRC,
        /// The shortest PPM sync.
        PpmSyncMs = ppm::PPM_SYNC_MS,
        /// The shortest PPM channel.
        PpmChannelMinMs = ppm::PPM_CHANNEL_MIN_MS,
        /// The longest PPM channel.
        PpmChannelMaxMs = ppm::PPM_CHANNEL_MAX_MS,
        /// The PPM centre of travel.
        PpmCentreMs = ppm::PPM_CENTRE_MS,
        /// The PPM row interval.
        PpmRowS = ppm::PPM_ROW_S,
    }
}

sdrkit_core::settings! {
    /// The settings of the `rc_links` stack: those of the reader and of the PPM test signal.
    pub enum RcStackSetting {
        /// The links.
        Submode = SUBMODE,
        /// The burst threshold.
        ThresholdDb = THRESHOLD_DB,
        /// The FlySky bit order.
        FlyskyBitOrder = FLYSKY_BIT_ORDER,
        /// The DSM chip polarity.
        DsmChipPolarity = DSM_CHIP_POLARITY,
        /// The S-FHSS CRC.
        SfhssCrc = SFHSS_CRC,
        /// The shortest PPM sync.
        PpmSyncMs = ppm::PPM_SYNC_MS,
        /// The shortest PPM channel.
        PpmChannelMinMs = ppm::PPM_CHANNEL_MIN_MS,
        /// The longest PPM channel.
        PpmChannelMaxMs = ppm::PPM_CHANNEL_MAX_MS,
        /// The PPM centre of travel.
        PpmCentreMs = ppm::PPM_CENTRE_MS,
        /// The PPM row interval.
        PpmRowS = ppm::PPM_ROW_S,
        /// The PPM channels of the test signal.
        GenChannelsMs = ppm::GEN_CHANNELS_MS,
        /// The PPM frame.
        GenFrameMs = ppm::GEN_FRAME_MS,
        /// The PPM pulse.
        GenPulseUs = ppm::GEN_PULSE_US,
        /// The PPM jitter.
        GenJitterUs = ppm::GEN_JITTER_US,
        /// The PPM transmitter clock.
        GenPpmClockPpm = ppm::GEN_PPM_CLOCK_PPM,
        /// The PPM keying depth.
        GenPpmDepth = ppm::GEN_PPM_DEPTH,
        /// The PPM polarity.
        GenPpmInvert = ppm::GEN_PPM_INVERT,
    }
}

/// What the stack publishes.
pub fn declaration() -> ResultsDecl {
    crate::typed::shape("rc_links").results_decl()
}

/// A frame decoded from a burst.
#[derive(Clone, Debug, PartialEq)]
pub struct RcFrame {
    /// The link.
    pub link: Link,
    /// The transmitter id as text.
    pub id: String,
    /// The type text.
    pub kind: String,
    /// The fields.
    pub detail: String,
    /// The bytes after the pattern.
    pub bytes: Vec<u8>,
    /// The frequency, Hz from the input centre.
    pub offset_hz: f64,
    /// The stream index of the first bit after the pattern.
    pub sample: u64,
}

/// The patterns of the FSK links, as bits.
struct Patterns {
    frsky: Vec<u8>,
    flysky_msb: Vec<u8>,
    flysky_lsb: Vec<u8>,
}

/// The reader of the bursts of every link.
pub struct RcReader {
    front: FskFront,
    dsm_mixer: Option<BurstMixer>,
    dsm: dsm::DsmSearch,
    pat: Patterns,
    bits: Vec<u8>,
    bytes: Vec<u8>,
    base: Vec<Complex32>,
    disc: Vec<f32>,
    chips: Vec<u8>,
    /// The decoded DSM packets: their first and last stream index.
    dsm_seen: Vec<(u64, u64)>,
    /// The radio centre, Hz (0 when not known).
    pub centre_hz: f64,
    /// The FlySky bit order.
    pub bit_order: BitOrder,
    /// The S-FHSS CRC.
    pub sfhss_crc: SfhssCrc,
    /// The DSM chip polarity.
    pub chip_polarity: ChipPolarity,
}

impl RcReader {
    /// A reader for input at `fs`.
    pub fn new(fs: f64) -> RcReader {
        let bits = |b: &[u8], lsb: bool| {
            let mut v = Vec::new();
            crate::fsklink::bits_of(b, lsb, &mut v);
            v
        };
        let dsm_decim = ((fs / 4e6).floor() as usize).max(1);
        let taps = (4.0 * fs / DSM_CUTOFF_HZ).ceil() as usize;
        RcReader {
            front: FskFront::new(fs),
            dsm_mixer: BurstMixer::new(fs, dsm_decim, DSM_CUTOFF_HZ.min(fs / 2.0), taps).ok(),
            dsm: dsm::DsmSearch::default(),
            pat: Patterns {
                frsky: bits(&frsky::PATTERN, false),
                flysky_msb: bits(&flysky::PATTERN, false),
                flysky_lsb: bits(&flysky::PATTERN, true),
            },
            bits: Vec::with_capacity(2048),
            bytes: Vec::with_capacity(256),
            base: Vec::new(),
            disc: Vec::new(),
            chips: Vec::new(),
            dsm_seen: Vec::with_capacity(16),
            centre_hz: 0.0,
            bit_order: BitOrder::Both,
            sfhss_crc: SfhssCrc::Either,
            chip_polarity: ChipPolarity::High,
        }
    }

    /// The input rate.
    pub fn rate_hz(&self) -> f64 {
        self.front.rate_hz()
    }

    /// The links of `wanted` that the input rate allows.
    pub fn usable(&self, wanted: &[Link]) -> Vec<Link> {
        wanted
            .iter()
            .copied()
            .filter(|l| self.rate_hz() >= l.least_rate_hz())
            .collect()
    }

    /// The first frame of burst `b` on the links `links`.
    pub fn read(&mut self, b: &Burst, links: &[Link]) -> Option<RcFrame> {
        for &l in links {
            if self.rate_hz() < l.least_rate_hz() {
                continue;
            }
            let f = match l {
                Link::Frsky => self.frsky(b),
                Link::Sfhss => self.sfhss(b),
                Link::Flysky => self.flysky(b),
                Link::Dsm => self.dsm(b),
            };
            if f.is_some() {
                return f;
            }
        }
        None
    }

    /// Demodulates at `rate`, finds `pattern` and slices `max_bytes` bytes after it in the order `lsb`. Returns the frequency
    /// and the sample of the first bit after the pattern.
    fn fsk(
        &mut self,
        b: &Burst,
        rate: f64,
        which: u8,
        polarity: Polarity,
        max_bytes: usize,
        lsb: bool,
    ) -> Option<(f64, u64, usize)> {
        if !self.front.demod(b, rate, 0.0, None) {
            return None;
        }
        let pattern = match which {
            0 => &self.pat.frsky,
            1 => &self.pat.flysky_msb,
            _ => &self.pat.flysky_lsb,
        };
        let hit = self.front.find(pattern, 1, polarity)?;
        let n = self.front.bits(&hit, 8 * max_bytes, &mut self.bits);
        bytes_of(&self.bits, lsb, &mut self.bytes);
        Some((
            b.offset_hz + self.front.cfo_hz(&hit),
            self.front.sample_of(b, &hit, 0),
            n,
        ))
    }

    fn frsky(&mut self, b: &Burst) -> Option<RcFrame> {
        let rates = [frsky::D8_RATE, frsky::D16_RATES[0], frsky::D16_RATES[1]];
        for (i, &r) in rates.iter().enumerate() {
            let max = if i == 0 { frsky::D8_LEN } else { frsky::D16_MAX };
            let Some((f, sample, _)) = self.fsk(b, r, 0, Polarity::Normal, max, false) else {
                continue;
            };
            let p = if i == 0 {
                frsky::parse_d8(&self.bytes)
            } else {
                frsky::parse_d16(&self.bytes)
            };
            if let Some(p) = p {
                let n = match p {
                    frsky::Frsky::D16 { len, .. } => len,
                    _ => frsky::D8_LEN,
                };
                return Some(RcFrame {
                    link: Link::Frsky,
                    id: format!("{:04X}", p.id()),
                    kind: p.kind().to_string(),
                    detail: format!("{}, {} Bd", p.detail(), r),
                    bytes: self.bytes[..n.min(self.bytes.len())].to_vec(),
                    offset_hz: f,
                    sample,
                });
            }
        }
        None
    }

    fn sfhss(&mut self, b: &Burst) -> Option<RcFrame> {
        let (f, sample, _) = self.fsk(b, sfhss::RATE, 0, Polarity::Normal, sfhss::LEN, false)?;
        let p = sfhss::parse(&self.bytes, self.sfhss_crc)?;
        Some(RcFrame {
            link: Link::Sfhss,
            id: format!("{:04X}", p.id),
            kind: p.kind().to_string(),
            detail: format!("{}, {}", p.detail(), p.crc.label()),
            bytes: self.bytes[..sfhss::LEN].to_vec(),
            offset_hz: f,
            sample,
        })
    }

    fn flysky(&mut self, b: &Burst) -> Option<RcFrame> {
        let orders: &[bool] = match self.bit_order {
            BitOrder::Both => &[false, true],
            BitOrder::Msb => &[false],
            BitOrder::Lsb => &[true],
        };
        for &lsb in orders {
            let which = if lsb { 2 } else { 1 };
            let Some((f, sample, n)) =
                self.fsk(b, flysky::RATE, which, Polarity::Either, flysky::AFHDS2A_LEN, lsb)
            else {
                continue;
            };
            let long = n >= 8 * flysky::AFHDS2A_LEN;
            let Some(p) = flysky::parse(&self.bytes, long) else {
                continue;
            };
            let id = format!("{:08X}", p.id);
            let order = if lsb { "least significant bit first" } else { "most significant bit first" };
            return Some(RcFrame {
                link: Link::Flysky,
                id,
                kind: p.kind.text().to_string(),
                detail: if p.detail().is_empty() {
                    order.to_string()
                } else {
                    format!("{}, {order}", p.detail())
                },
                bytes: self.bytes[..p.len.min(self.bytes.len())].to_vec(),
                offset_hz: f,
                sample,
            });
        }
        None
    }

    /// The centres a DSM burst is tried at: its own, the nearest 1 MHz channel (when the radio centre is known and that is more
    /// than 50 kHz away), and ±400 kHz.
    fn dsm_centres(&self, b: &Burst) -> [Option<f64>; 4] {
        let mut c = [Some(b.offset_hz), None, Some(b.offset_hz - 400e3), Some(b.offset_hz + 400e3)];
        if self.centre_hz > 0.0 {
            let abs = self.centre_hz + b.offset_hz;
            let ch = (abs / 1e6).round() * 1e6 - self.centre_hz;
            if (ch - b.offset_hz).abs() > 50e3 {
                c[1] = Some(ch);
            }
        }
        c
    }

    fn dsm(&mut self, b: &Burst) -> Option<RcFrame> {
        if b.width_hz >= DSM_MAX_WIDTH_HZ {
            return None;
        }
        let first = b.start;
        let last = b.start + b.samples.len() as u64;
        if self.dsm_seen.iter().any(|&(a, z)| first >= a && first < z) {
            return None;
        }
        let centres = self.dsm_centres(b);
        for c in centres.into_iter().flatten() {
            if let Some(f) = self.dsm_at(b, c) {
                if self.dsm_seen.len() == self.dsm_seen.capacity() {
                    self.dsm_seen.remove(0);
                }
                self.dsm_seen.push((f.sample, last));
                return Some(f);
            }
        }
        None
    }

    fn dsm_at(&mut self, b: &Burst, centre: f64) -> Option<RcFrame> {
        let m = self.dsm_mixer.as_mut()?;
        let out_rate = m.out_rate_hz();
        if out_rate < DSM_MIN_WORK_HZ {
            return None;
        }
        m.run(&b.samples, b.start, centre, &mut self.base);
        self.disc.clear();
        self.disc.extend(self.base.windows(2).map(|w| {
            let z = w[1] * w[0].conj();
            sdrkit_algo_dsp::approx::atan2(z.im, z.re)
        }));
        let n = self.disc.len();
        if n < 64 {
            return None;
        }
        // The carrier: the mean of the discriminator over the middle of the burst.
        let mid = &self.disc[n / 10..n - n / 10];
        let thr = mid.iter().sum::<f32>() / mid.len().max(1) as f32;
        let cfo = f64::from(thr) * out_rate / std::f64::consts::TAU;
        if cfo.abs() > DSM_MAX_CFO_HZ {
            return None;
        }
        let sps = out_rate / dsm::CHIP_RATE;
        let phases = sps.round().max(1.0) as usize;
        let mut best: Option<(u32, dsm::Found, f64)> = None;
        for ph in 0..phases {
            self.chips.clear();
            let mut k = 0usize;
            loop {
                let a = (ph as f64 + k as f64 * sps).round() as usize;
                let z = (ph as f64 + (k + 1) as f64 * sps).round() as usize;
                if z > n || z <= a {
                    break;
                }
                let v = self.disc[a..z].iter().sum::<f32>() / (z - a) as f32;
                self.chips
                    .push(u8::from((v > thr) == (self.chip_polarity == ChipPolarity::High)));
                k += 1;
            }
            if let Some(f) = self.dsm.find(&self.chips) {
                let score = f.sop_errors + f.worst;
                if best.as_ref().is_none_or(|b| score < b.0) {
                    best = Some((score, f, ph as f64));
                }
            }
        }
        let (_, f, ph) = best?;
        let data = dsm::parse_data(&f.data)?;
        let decim = b.samples.len() as f64 / self.base.len().max(1) as f64;
        let sample = b.start + ((ph + f.at as f64 * sps) * decim) as u64;
        let abs = centre + cfo;
        let (kind, id) = match self.dsm_type(abs, f.set.row) {
            Some(true) => ("DSMX channels", format!("{:02X}{:02X}", data.head[0], data.head[1])),
            Some(false) => ("DSM2 channels", format!("{:02X}{:02X}", !data.head[0], !data.head[1])),
            None => ("DSM channels", format!("{:02X}{:02X}", data.head[0], data.head[1])),
        };
        let mut detail = String::new();
        for (i, (idx, v)) in data.channels.iter().enumerate() {
            let _ = write!(detail, "{}ch{} {v}", if i > 0 { ", " } else { "" }, idx + 1);
        }
        let _ = write!(
            detail,
            "; {} bits, {} words{}, pn row {}, SOP column {}, CRC not checked",
            data.bits,
            data.words,
            if data.second { ", second frame" } else { "" },
            f.set.row,
            f.set.col
        );
        let mut bytes = Vec::with_capacity(19);
        bytes.push(16);
        bytes.extend_from_slice(&f.data);
        bytes.extend_from_slice(&f.crc);
        Some(RcFrame {
            link: Link::Dsm,
            id,
            kind: kind.to_string(),
            detail,
            bytes,
            offset_hz: abs,
            sample,
        })
    }

    /// DSMX (true) or DSM2 (false) by the pn row of the rf channel at `offset_hz`; `None` without the radio centre or when the
    /// row fits neither.
    fn dsm_type(&self, offset_hz: f64, row: u8) -> Option<bool> {
        if self.centre_hz <= 0.0 {
            return None;
        }
        let rf = ((self.centre_hz + offset_hz - 2400e6) / 1e6).round();
        if !(0.0..=255.0).contains(&rf) {
            return None;
        }
        let rf = rf as u8;
        if dsm::pn_row(rf, true) == row {
            Some(true)
        } else if dsm::pn_row(rf, false) == row {
            Some(false)
        } else {
            None
        }
    }
}

/// The `rc_links.rx` decoder: bursts, the links and the tables.
pub struct RcDecoder {
    submode: Submode,
    threshold_db: f64,
    bit_order: BitOrder,
    sfhss_crc: SfhssCrc,
    chip_polarity: ChipPolarity,
    rate: f64,
    centre_hz: f64,
    tuned_hz: f64,
    origin_s: f64,
    fresh: bool,
    det: Option<BandDetector>,
    reader: Option<RcReader>,
    links: Vec<Link>,
    pool: Vec<Burst>,
    used: usize,
    held: LinkHeld,
    snr_db: Option<f64>,
    now: f64,
    last_frame: f64,
    live: bool,
    clock: Option<Clock>,
    removed: Vec<String>,
    events: Vec<Timed<RcLinkSeen>>,
    summary: String,
    dirty: bool,
    cleared: bool,
    started: bool,
    ppm_rules: ppm::PpmRules,
    ppm: Option<ppm::PpmReader>,
    ppm_events: Vec<sdrkit_core::Event>,
    ppm_serial: u64,
    ppm_cleared: bool,
    ppm_pending: Vec<ppm::PpmFrame>,
}

impl Default for RcDecoder {
    fn default() -> Self {
        RcDecoder {
            submode: Submode::All,
            threshold_db: THRESHOLD_DB.default,
            bit_order: BitOrder::Both,
            sfhss_crc: SfhssCrc::Either,
            chip_polarity: ChipPolarity::High,
            rate: 0.0,
            centre_hz: 0.0,
            tuned_hz: 0.0,
            origin_s: 0.0,
            fresh: true,
            det: None,
            reader: None,
            links: Vec::new(),
            pool: Vec::new(),
            used: 0,
            held: LinkHeld::default(),
            snr_db: None,
            now: 0.0,
            last_frame: f64::NEG_INFINITY,
            live: false,
            clock: None,
            removed: Vec::new(),
            events: Vec::new(),
            summary: String::with_capacity(200),
            dirty: true,
            cleared: false,
            started: false,
            ppm_rules: ppm::PpmRules::default(),
            ppm: None,
            ppm_events: Vec::new(),
            ppm_serial: 0,
            ppm_cleared: false,
            ppm_pending: Vec::new(),
        }
    }
}

impl RcDecoder {
    /// The tables and counters.
    pub fn held(&self) -> &LinkHeld {
        &self.held
    }

    fn build(&mut self) -> Result<(), ConfigError> {
        self.fresh = true;
        let offset = self.tuned_hz - self.centre_hz;
        if self.submode != Submode::Ppm72 {
            self.ppm = None;
        } else if let Some(p) = self.ppm.as_mut().filter(|p| p.is_for(self.rate, offset)) {
            p.rules = self.ppm_rules;
        } else {
            self.ppm = Some(ppm::PpmReader::new(self.rate, offset, self.ppm_rules));
        }
        self.det = Some(
            BandDetector::new(band_config(self.rate, self.threshold_db))
                .map_err(ConfigError::from)?,
        );
        if self.reader.as_ref().is_none_or(|r| r.rate_hz() != self.rate) {
            self.reader = Some(RcReader::new(self.rate));
        }
        if let Some(r) = self.reader.as_mut() {
            r.centre_hz = self.centre_hz.max(0.0);
            r.bit_order = self.bit_order;
            r.sfhss_crc = self.sfhss_crc;
            r.chip_polarity = self.chip_polarity;
            self.links = r.usable(self.submode.links());
        }
        Ok(())
    }

    fn stamp(&self, t: f64) -> Stamp {
        match self.clock.as_ref().and_then(|c| wall(self.live, c)) {
            Some(w) => Stamp::at(t).with_wall(w),
            None => Stamp::at(t),
        }
    }

    fn burst(&mut self, i: usize) {
        self.held.counters.bursts += 1;
        let b = std::mem::take(&mut self.pool[i]);
        let got = self.reader.as_mut().and_then(|r| r.read(&b, &self.links));
        let snr = f64::from(b.peak_db);
        self.pool[i] = b;
        let Some(f) = got else {
            self.held.counters.without_frame += 1;
            return;
        };
        let t = self.origin_s + f.sample as f64 / self.rate.max(1.0);
        let at = self.stamp(t);
        let freq = if self.centre_hz > 0.0 {
            self.centre_hz + f.offset_hz
        } else {
            f.offset_hz
        };
        let h = Heard {
            t,
            link: f.link.name(),
            id: f.id.clone(),
            kind: f.kind.clone(),
            detail: f.detail.clone(),
            bytes: &f.bytes,
            freq_hz: freq,
            snr_db: snr,
            encrypted: false,
            position: None,
        };
        let taken = self.held.take(&h, at);
        if taken.new_link {
            self.events.push(Timed {
                time: t,
                value: RcLinkSeen(NewLink {
                    link: f.link.name().to_string(),
                    id: f.id,
                    position: None,
                }),
            });
        }
        self.snr_db = Some(snr);
        self.last_frame = t;
        self.dirty = true;
    }

    fn covered(&self) -> String {
        let dropped: Vec<&str> = self
            .submode
            .links()
            .iter()
            .filter(|l| !self.links.contains(l))
            .map(|l| l.name())
            .collect();
        let on: Vec<&str> = self.links.iter().map(|l| l.name()).collect();
        let mut s = format!("±{:.2} MHz around the centre; {}", self.rate / 2e6, on.join(", "));
        if !dropped.is_empty() {
            let _ = write!(s, "; not at this sample rate: {}", dropped.join(", "));
        }
        s
    }

    fn summary(&self, out: &mut String) {
        out.clear();
        let c = &self.held.counters;
        // Writing into a `String` cannot fail.
        let _ = if !self.started {
            write!(out, "RC links: waiting for samples")
        } else if self.links.is_empty() {
            write!(out, "RC links: no link fits this sample rate")
        } else if c.frames == 0 {
            write!(out, "RC links: searching ±{:.2} MHz, {} bursts, no frame yet", self.rate / 2e6, c.bursts)
        } else {
            write!(
                out,
                "RC links: {} transmitters, {} frames; searching ±{:.2} MHz",
                self.held.links.len(),
                c.frames,
                self.rate / 2e6
            )
        };
    }
}

impl Decoder for RcDecoder {
    type Input = Complex32;
    fn name(&self) -> &'static str {
        "rc_links.rx"
    }
    fn outputs(&self) -> Vec<Port> {
        vec![
            Port::typed::<RcLinkSeen>("links"),
            Port::new("events", Kind::Messages).rate_from(sdrkit_graph::RateFrom::Fixed(0.0)),
        ]
    }
    fn settings(&self) -> Arc<sdrkit_core::Schema> {
        RcSetting::schema()
    }
    fn values(&self) -> Values {
        values_of([
            (RcSetting::Submode, Value::Choice(self.submode.index())),
            (RcSetting::ThresholdDb, Value::F64(self.threshold_db)),
            (RcSetting::FlyskyBitOrder, Value::Choice(self.bit_order.index())),
            (
                RcSetting::DsmChipPolarity,
                Value::Choice(self.chip_polarity.index()),
            ),
            (RcSetting::SfhssCrc, Value::Choice(self.sfhss_crc.index())),
            (RcSetting::PpmSyncMs, Value::F64(self.ppm_rules.sync_s * 1e3)),
            (RcSetting::PpmChannelMinMs, Value::F64(self.ppm_rules.channel_s.0 * 1e3)),
            (RcSetting::PpmChannelMaxMs, Value::F64(self.ppm_rules.channel_s.1 * 1e3)),
            (RcSetting::PpmCentreMs, Value::F64(self.ppm_rules.centre_s * 1e3)),
            (RcSetting::PpmRowS, Value::F64(self.ppm_rules.row_s)),
        ])
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<(), SettingError> {
        match RcSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            RcSetting::Submode => self.submode = SUBMODE.read(id, v)?,
            RcSetting::ThresholdDb => self.threshold_db = THRESHOLD_DB.read(id, v)?,
            RcSetting::FlyskyBitOrder => self.bit_order = FLYSKY_BIT_ORDER.read(id, v)?,
            RcSetting::DsmChipPolarity => self.chip_polarity = DSM_CHIP_POLARITY.read(id, v)?,
            RcSetting::SfhssCrc => self.sfhss_crc = SFHSS_CRC.read(id, v)?,
            RcSetting::PpmSyncMs => self.ppm_rules.sync_s = ppm::PPM_SYNC_MS.read(id, v)? * 1e-3,
            RcSetting::PpmChannelMinMs => self.ppm_rules.channel_s.0 = ppm::PPM_CHANNEL_MIN_MS.read(id, v)? * 1e-3,
            RcSetting::PpmChannelMaxMs => self.ppm_rules.channel_s.1 = ppm::PPM_CHANNEL_MAX_MS.read(id, v)? * 1e-3,
            RcSetting::PpmCentreMs => self.ppm_rules.centre_s = ppm::PPM_CENTRE_MS.read(id, v)? * 1e-3,
            RcSetting::PpmRowS => self.ppm_rules.row_s = ppm::PPM_ROW_S.read(id, v)?,
        }
        if self.rate > 0.0 {
            // A bad rate was refused at configure, so the detector builds again.
            let _ = self.build();
        }
        Ok(())
    }
    fn results(&self) -> Published {
        Published::from_decl(Arc::new(declaration()))
    }
    fn configure(&mut self, input: &Format) -> Result<Vec<Format>, ConfigError> {
        if input.rate < MIN_RATE_HZ {
            return Err(ConfigError::RateTooLow {
                need: MIN_RATE_HZ,
                got: input.rate,
            });
        }
        self.rate = input.rate;
        self.centre_hz = input.centre_hz;
        self.tuned_hz = input.tuned_hz;
        self.build()?;
        Ok(vec![
            Format {
                rate: 0.0,
                frame_len: 0,
                media: None,
                ..input.with_kind(Kind::typed::<RcLinkSeen>())
            },
            Format::new(Kind::Messages, 0.0),
        ])
    }
    fn retune(&mut self, input: &Format) {
        self.centre_hz = input.centre_hz;
        self.tuned_hz = input.tuned_hz;
        let _ = self.build();
    }
    fn attach(&mut self, ctx: &RunContext) {
        self.live = ctx.wall_clock();
        self.clock = Some(ctx.clock.clone());
    }
    fn process(&mut self, x: &[Complex32], first: u64, out: &mut Outbox) {
        self.started = true;
        self.now = (first + x.len() as u64) as f64 / self.rate.max(1.0);
        if std::mem::take(&mut self.fresh) {
            self.origin_s = first as f64 / self.rate.max(1.0);
        }
        if let Some(p) = self.ppm.as_mut() {
            p.process(x);
            for f in p.rows.drain(..) {
                self.ppm_events.push(f.event(self.tuned_hz));
                self.ppm_pending.push(f);
            }
            if !self.ppm_events.is_empty() {
                out.messages(1).append(&mut self.ppm_events);
            }
            return;
        }
        self.used = 0;
        if let Some(d) = self.det.as_mut() {
            let (pool, used) = (&mut self.pool, &mut self.used);
            d.process(x, |b| {
                if *used == pool.len() {
                    pool.push(Burst::default());
                }
                pool[*used].take(b);
                *used += 1;
            });
        }
        for i in 0..self.used {
            self.burst(i);
        }
        if !self.events.is_empty() {
            out.items::<RcLinkSeen>(0).append(&mut self.events);
        }
    }
    fn commands(&self) -> &'static [&'static Command] {
        LinkCommand::COMMANDS
    }
    fn invoke(&mut self, id: CommandId, args: &Values) -> Result<Option<Record>, CommandError> {
        match LinkCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            LinkCommand::Remove => {
                let table = declaration()
                    .table_id(LINKS.key)
                    .unwrap_or(sdrkit_core::TableId(0));
                self.held.remove(args, table, &mut self.removed)?;
                self.dirty = true;
            }
        }
        Ok(None)
    }
    fn report(&mut self, p: &mut Published) {
        let recent = self.now - self.last_frame <= RECENT_S;
        for k in self.removed.drain(..) {
            p.remove_row(&LINKS, &k);
        }
        if std::mem::take(&mut self.cleared) {
            p.results_mut().clear_log();
            for t in [LINKS.def(), FRAMES.def()] {
                p.clear_rows(t);
            }
        }
        let covered = self.covered();
        p.put(&self.held.fields(recent, &covered, self.snr_db, NOT_CONFIRMED));
        if std::mem::take(&mut self.ppm_cleared) {
            p.clear_rows(&RC_LINKS_PPM_FRAMES);
        }
        for f in self.ppm_pending.drain(..) {
            self.ppm_serial += 1;
            let values = ppm::values_text(&f.channels);
            RC_LINKS_PPM_FRAMES.put(p, &format!("{:06}", self.ppm_serial), &f.row(&values));
        }
        let pp = self.ppm.as_ref();
        let last = pp.and_then(|r| r.last.as_ref());
        let values = last.map(|f| ppm::values_text(&f.channels));
        let pct = last.map(|f| ppm::percent_text(&f.channels, self.ppm_rules.centre_s));
        p.put(&RcLinksPpmFields {
            channels: last.map(|f| f.channels.len() as i64),
            frame_ms: last.map(|f| f.frame_s * 1e3),
            frame_rate_hz: last.map(|f| 1.0 / f.frame_s),
            pulse_state: pp.map(ppm::PpmReader::pulse_state),
            channel_ms: values.as_deref(),
            channel_pct: pct.as_deref(),
            ppm_frames_ok: pp.map_or(0, |r| r.good),
            ppm_frames_bad: pp.map_or(0, |r| r.bad),
        });
        if let Some(r) = pp {
            let now = r.now();
            let heard = r.last.as_ref().is_some_and(|f| now - f.time_s <= 8.0);
            let lock = if heard {
                LockState::Locked
            } else if r.last_onset.is_some_and(|t| now - t <= 2.0) {
                LockState::Searching
            } else {
                LockState::Off
            };
            let summary = match r.last.as_ref() {
                Some(f) => format!("RC PPM: {} channels, frame {:.1} ms", f.channels.len(), f.frame_s * 1e3),
                None => "RC PPM: no PPM frame yet".to_string(),
            };
            p.results_mut().update_status(|s| {
                let changed = s.lock_state != lock || s.summary != summary;
                if changed {
                    s.lock_state = lock;
                    s.summary.clone_from(&summary);
                }
                changed
            });
            return;
        }
        if std::mem::take(&mut self.dirty) {
            self.held.rows(self.now, false, p);
        }
        let lock = if !self.started {
            LockState::Off
        } else if recent {
            LockState::Locked
        } else {
            LockState::Searching
        };
        let mut summary = std::mem::take(&mut self.summary);
        self.summary(&mut summary);
        p.results_mut().update_status(|s| {
            let changed = s.lock_state != lock || s.summary != summary;
            if changed {
                s.lock_state = lock;
                s.summary.clone_from(&summary);
            }
            changed
        });
        self.summary = summary;
    }
    fn report_interval_s(&self) -> f64 {
        REPORT_S
    }
    fn clear(&mut self) {
        self.held.clear();
        if let Some(p) = self.ppm.as_mut() {
            p.clear();
        }
        self.ppm_pending.clear();
        self.ppm_cleared = true;
        self.cleared = true;
        self.dirty = true;
    }
    fn save_state(&self) -> Option<StateBlob> {
        self.held.save_state(self.now).ok()
    }
    fn restore_state(&mut self, state: &StateBlob) -> Result<(), StateError> {
        self.held.restore_state(state, self.now)?;
        self.cleared = true;
        self.dirty = true;
        Ok(())
    }
}

/// The `rc_links` receiver.
///
/// # Errors
///
/// [`GraphError`] if the graph does not build or a value of `cfg` is refused.
pub fn receiver(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    let graph = GraphBuilder::new()
        .add("rx", DecoderComponent::new(RcDecoder::default()))
        .input("rx")
        .output("rx.links")
        .output("rx.events")
        .clock(REPORT_S)
        .status_from("rx")
        .catalog(&Catalog::of(cfg)?)
        .build()?;
    StackGraph::of(crate::typed::shape(ID), graph)
        .min_rate(MIN_RATE_HZ, &min_rate_message("RC links", MIN_RATE_HZ))
        .route(SUBMODE.key(), &["rx.submode"])?
        .route(THRESHOLD_DB.key(), &["rx.threshold_db"])?
        .route(FLYSKY_BIT_ORDER.key(), &["rx.flysky_bit_order"])?
        .route(DSM_CHIP_POLARITY.key(), &["rx.dsm_chip_polarity"])?
        .route(SFHSS_CRC.key(), &["rx.sfhss_crc"])?
        .route(ppm::PPM_SYNC_MS.key(), &["rx.ppm_sync_ms"])?
        .route(ppm::PPM_CHANNEL_MIN_MS.key(), &["rx.ppm_channel_min_ms"])?
        .route(ppm::PPM_CHANNEL_MAX_MS.key(), &["rx.ppm_channel_max_ms"])?
        .route(ppm::PPM_CENTRE_MS.key(), &["rx.ppm_centre_ms"])?
        .route(ppm::PPM_ROW_S.key(), &["rx.ppm_row_s"])?
        .command("clear", sdrkit_graph::CommandTarget::every("clear"))
        .command("remove", sdrkit_graph::CommandTarget::of("rx", "remove"))
        .with_settings(&cfg.settings)
        .build()
}

fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    Ok(Box::new(receiver(cfg)?))
}

fn make_generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    let id = cfg.settings.schema().id(SUBMODE.key()).ok();
    let sub = id.and_then(|i| cfg.settings.value(i).and_then(|v| SUBMODE.read(i, v).ok()));
    if sub == Some(Submode::Ppm72) {
        return Ok(Box::new(ppm::PpmGenerator::new(cfg.rate_hz, &cfg.settings, cfg.seed)));
    }
    Ok(Box::new(generator::from_config(cfg)?))
}

/// The `rc_links` stack. Its default frequency is the centre of the test signal; the session gives the real one.
pub static RC_LINKS: StackDescriptor = StackDescriptor {
    id: ID,
    name: "RC links",
    group: crate::GROUP,
    blurb: "2.4 GHz radio-control links from bursts anywhere in the capture: FrSky, FlySky, Futaba S-FHSS and Spektrum DSM transmitter ids and channel values. Hops are not followed.",
    tuning: Tuning {
        no_offset_tuning: true,
        ..Tuning::anywhere(generator::TEST_CENTRE_HZ, 8e6, MIN_RATE_HZ, 8e6)
    },
    submodes: &[],
    settings: RcStackSetting::schema,
    make_receiver,
    make_generator: Some(make_generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[],
    auto_gain: AutoGain::Radio,
    verify: Some(&crate::RC_LINKS_VERIFY),
    benchmarks: Some(benchmarks),
};

/// The benchmark of the receiver: 1 s of the test signal at 8 Msps.
pub fn benchmarks() -> Vec<sdrkit_core::Benchmark> {
    vec![
        sdrkit_core::Benchmark {
            name: "rc_links.receiver_8m",
            run: |s| crate::bench_receiver(s, &RC_LINKS, 1.0),
        },
        sdrkit_core::Benchmark {
            name: "rc_links.ppm72_1m",
            run: ppm_bench,
        },
    ]
}

/// The PPM reader at 1 Msps: the front end to 166.7 ksps and the slicer, on 2 s of four channels.
fn ppm_bench(seconds: f64) -> sdrkit_core::BenchResult {
    use sdrkit_core::{InputInfo, Output, Values};
    let mut v = Values::defaults(&RcStackSetting::schema());
    if let Ok(id) = v.schema().id(SUBMODE.key()) {
        let _ = v.set(id, &Value::Choice(Submode::Ppm72.index()));
    }
    let rate = 1e6;
    let mut g = ppm::PpmGenerator::new(rate, &v, 1);
    let mut x = vec![Complex32::default(); 2_000_000];
    g.generate(&mut x);
    let Ok(mut rx) = make_receiver(&StackConfig::new(v)) else {
        return sdrkit_core::BenchResult::default();
    };
    let _ = rx.configure(&InputInfo::new(rate, 72.35e6, 72.35e6).recording());
    let mut out = Output::default();
    sdrkit_core::bench::measure(seconds, x.len(), 2.0, || {
        for b in x.chunks(65_536) {
            out.clear();
            rx.feed(b, &mut out);
        }
    })
}
