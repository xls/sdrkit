//! CubeSat telemetry (`cubesat`): AX.25 (AFSK, G3RUH), AX100 Mode 5, CCSDS Reed-Solomon and the burst framings (Geoscan,
//! Lucky-7, Spino, Erminaz, USP, NGHam, U482C, Mobitex, CCSDS concatenated) on an FM channel, named from a satellite table. The
//! `table` receiver runs every framing at the dial, and the `smart` search runs the same demodulators on the narrow carriers it
//! finds. A named submode runs one burst framing alone.

pub mod demod;
pub mod display;
pub mod framing;
pub mod framings;
pub mod gen;
pub mod tx;

use demod::{DemodConfig, ExtraAx25, FmDemods, Framing, FramingCounts, RawFrame, Submode};
use display::CsDisplay;
use framings::{ConcatCode, MobitexVariant};
use sdrkit_algo_dsp::spectrum::{percentile, Scaling, SpectrumAnalyzer, SpectrumConfig};
use sdrkit_algo_dsp::window::Window;
use sdrkit_algo_fec::rs::Basis;
use sdrkit_core::publish::field;
use sdrkit_core::verify::{CheckLine, VerifyCase, VerifyHooks, VerifyOptions};
use sdrkit_core::SignalType;
use sdrkit_core::{
    AutoGain, Choice, Complex32, ConfigError, DeclareSetting, GenConfig, Generator, LockProfile,
    Receiver, ResultsDecl, Schema, Setting, SettingError, SettingId, SettingSet, StackConfig,
    StackDescriptor, SubMode, Tuning, Value, Values,
};
use sdrkit_graph::{
    values_of, Catalog, Decoder, DecoderComponent, Format, GraphBuilder, GraphError, Item, Kind,
    Outbox, Port, Registry, StackGraph, Timed,
};
use std::sync::{Arc, OnceLock};

/// The stack id.
pub const ID: &str = "cubesat";
/// The stack name.
pub const NAME: &str = "CubeSat telemetry";
/// The lowest sample rate, Hz.
pub const MIN_RATE: f64 = 1e6;
/// The lowest sample rate of a named submode: twice the tune offset and half the shown width, Hz.
pub const MIN_RATE_NAMED: f64 = 130e3;
/// The dial of the test signal and of every submode. It is not the downlink of a satellite: the host gives the frequency.
pub const TEST_SIGNAL_HZ: f64 = 145.825e6;
/// Seconds between reports.
pub const REPORT_S: f64 = 0.25;
/// The satellite table the stack starts with.
pub const DEFAULT_TABLE: &str = "ISS (ARISS packet)|25544|145.825|AX.25 AFSK|1200|RS0ISS;NO-44 (PCSat)|26931|145.825|AX.25 AFSK|1200|";
/// A carrier within this distance of a channel keeps the channel, Hz.
const FOLLOW_HZ: f64 = 5e3;
/// The width the smart scan smooths over, Hz.
const SCAN_SMOOTH_HZ: f64 = 10e3;

/// A satellite table entry.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SatEntry {
    /// The satellite's name.
    pub name: String,
    /// Its NORAD catalogue number.
    pub norad: u32,
    /// Its downlink, MHz.
    pub mhz: f64,
    /// Its framing (`None` for any).
    pub framing: Option<Framing>,
    /// Its bit rate.
    pub baud: u32,
    /// Its callsign, without SSID (empty for none).
    pub call: String,
}

/// Reads a satellite table "name|norad|MHz|framing|baud|call;...". The framing is a key or a label of [`Framing`], or empty or
/// "auto" for any.
///
/// # Errors
///
/// The byte position of the first entry that is not of this form.
pub fn parse_table(s: &str) -> Result<Vec<SatEntry>, usize> {
    let mut out = Vec::new();
    let mut at = 0;
    for e in s.split(';') {
        let start = at;
        at += e.len() + 1;
        if e.trim().is_empty() {
            continue;
        }
        let f: Vec<&str> = e.split('|').map(str::trim).collect();
        if f.len() < 5 {
            return Err(start);
        }
        let framing = match f[3] {
            "" | "auto" => None,
            x => Some(Framing::parse(x).ok_or(start)?),
        };
        out.push(SatEntry {
            name: f[0].into(),
            norad: f[1].parse().map_err(|_| start)?,
            mhz: f[2].parse().map_err(|_| start)?,
            framing,
            baud: f[4].parse().map_err(|_| start)?,
            call: f.get(5).map(|c| c.to_string()).unwrap_or_default(),
        });
    }
    Ok(out)
}

/// Reads the satellite followed, "name|norad" (the number 0 when it is left out). Empty text follows none.
///
/// # Errors
///
/// The byte position of a number that does not read.
pub fn parse_tracked(t: &str) -> Result<Option<(String, u32)>, usize> {
    if t.trim().is_empty() {
        return Ok(None);
    }
    let (n, id) = t.split_once('|').unwrap_or((t, "0"));
    let id = id.trim().parse().map_err(|_| n.len() + 1)?;
    Ok(Some((n.trim().to_string(), id)))
}

/// A decoded frame with its fields and its satellite.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Named {
    /// The demodulated frame.
    pub raw: Option<RawFrame>,
    /// Signal time of the end of the frame, seconds.
    pub time: f64,
    /// The absolute carrier frequency, Hz.
    pub freq_hz: f64,
    /// The SNR of the carrier, dB (smart search only).
    pub snr_db: Option<f64>,
    /// The satellite.
    pub satellite: String,
    /// Its NORAD number (0 when not known).
    pub norad: u32,
    /// The name comes from the dial frequency rather than a callsign or the satellite followed.
    pub guess: bool,
    /// AX.25: source, destination, repeaters, frame type and PID.
    pub from: String,
    /// AX.25 destination.
    pub to: String,
    /// AX.25 repeaters, separated by commas.
    pub path: String,
    /// AX.25 frame type ("UI" ...).
    pub frame_type: String,
    /// AX.25 PID.
    pub pid: Option<u8>,
    /// The CSP header in words (AX100).
    pub csp: String,
    /// The payload: the AX.25 information field, the bytes after the CSP header, or the CCSDS data.
    pub payload: Vec<u8>,
    /// A text beacon in the payload.
    pub text: Option<String>,
}

/// What the receiver passes on.
#[derive(Clone, Debug)]
pub enum CsItem {
    /// A frame from a demodulator.
    Raw {
        /// The frame.
        frame: RawFrame,
        /// The carrier's offset from the input centre, Hz.
        carrier_hz: f64,
        /// The carrier's SNR, dB (smart search only).
        snr_db: Option<f64>,
    },
    /// A frame with its fields and its satellite.
    Named(Box<Named>),
    /// The receiver's state at a clock tick.
    Status {
        /// A carrier is there.
        carrier: bool,
        /// The carrier's offset from the dial, Hz.
        offset_hz: f64,
        /// The smart search's carriers: offset from the dial (Hz) and SNR (dB).
        carriers: Vec<(f64, f64)>,
        /// The sync matches and failed decodes of the burst framings.
        counts: Vec<(Framing, FramingCounts)>,
    },
}

impl Item for CsItem {
    const KIND: &'static str = "satcom.cubesat_item";
}

/// One FM channel the receiver runs: its offset from the input centre, its demodulators and the carrier's SNR.
struct Channel {
    offset: f64,
    demods: FmDemods,
    snr_db: Option<f64>,
    /// Signal time when the channel started, seconds.
    born_s: f64,
}

/// The receiver decoder (`satcom.cubesat_receiver`): the table receiver's FM channel at the dial, or the smart search's channels
/// on the carriers it finds.
pub struct CubesatRx {
    rate: f64,
    dial: f64,
    smart: bool,
    demod: DemodConfig,
    extra_text: String,
    max_channels: usize,
    min_snr_db: f64,
    chans: Vec<Channel>,
    found: Vec<RawFrame>,
    scan: Option<SpectrumAnalyzer>,
    scratch: Vec<f32>,
    smooth: Vec<f32>,
    carriers: Vec<(f64, f64)>,
    counts: Vec<(Framing, FramingCounts)>,
    time_s: f64,
}

impl Default for CubesatRx {
    fn default() -> Self {
        CubesatRx::new(Submode::All)
    }
}

impl CubesatRx {
    /// The receiver of `submode`.
    pub fn new(submode: Submode) -> Self {
        CubesatRx {
            rate: 0.0,
            dial: 0.0,
            smart: false,
            demod: DemodConfig {
                submode,
                ..DemodConfig::default()
            },
            extra_text: String::new(),
            max_channels: 4,
            min_snr_db: 8.0,
            chans: Vec::new(),
            found: Vec::new(),
            scan: None,
            scratch: Vec::new(),
            smooth: Vec::new(),
            carriers: Vec::new(),
            counts: Vec::new(),
            time_s: 0.0,
        }
    }

    fn build(&mut self) -> Result<(), ConfigError> {
        self.chans.clear();
        self.carriers.clear();
        if !self.smart {
            self.chans.push(Channel {
                offset: self.dial,
                demods: FmDemods::new(self.rate, self.dial, &self.demod)?,
                snr_db: None,
                born_s: self.time_s,
            });
            self.scan = None;
            return Ok(());
        }
        let n = ((self.rate / 500.0) as usize)
            .next_power_of_two()
            .clamp(1024, 16_384);
        let mut cfg = SpectrumConfig::new(n, self.rate);
        cfg.window = Window::Hann;
        cfg.scaling = Scaling::Power;
        cfg.frames_per_second = self.rate / n as f64;
        cfg.average = 8;
        self.scan = Some(SpectrumAnalyzer::new(cfg)?);
        Ok(())
    }

    /// The smart search's carrier scan: regions `min_snr_db` over the median, their power centroid and peak SNR. Channels follow
    /// the carriers.
    fn rescan(&mut self) -> Result<(), ConfigError> {
        let Some(sa) = self.scan.as_mut() else {
            return Ok(());
        };
        let p = sa.power();
        let n = p.len();
        let bin = self.rate / n as f64;
        let k = ((SCAN_SMOOTH_HZ / bin) as usize).max(1);
        self.smooth.clear();
        self.smooth.extend((0..n).map(|i| {
            let a = i.saturating_sub(k / 2);
            let b = (i + k / 2).min(n - 1);
            p[a..=b].iter().sum::<f32>() / (b - a + 1) as f32
        }));
        let sm = &self.smooth;
        let floor = percentile(sm, 0.5, &mut self.scratch).max(1e-30);
        let thr = floor * 10f32.powf(self.min_snr_db as f32 / 10.0);
        let mut found = Vec::new();
        let edge = n / 20;
        let mut i = edge;
        while i < n - edge {
            if sm[i] > thr {
                let s = i;
                while i < n && sm[i] > thr {
                    i += 1;
                }
                let (mut num, mut den) = (0.0, 0.0);
                for (j, v) in sm.iter().enumerate().take(i).skip(s) {
                    let w = (*v - floor) as f64;
                    num += w * (j as f64 - (n / 2) as f64) * bin;
                    den += w;
                }
                let peak = sm[s..i].iter().copied().fold(0.0, f32::max);
                found.push((num / den.max(1e-30), 10.0 * ((peak / floor) as f64).log10()));
            }
            i += 1;
        }
        found.sort_by(|a, b| b.1.total_cmp(&a.1));
        found.truncate(self.max_channels);
        let mut keep = Vec::new();
        for &(c, snr) in &found {
            match self
                .chans
                .iter()
                .position(|ch| (ch.offset - c).abs() < FOLLOW_HZ)
            {
                Some(i) => {
                    let mut ch = self.chans.remove(i);
                    ch.snr_db = Some(snr);
                    keep.push(ch);
                }
                None => keep.push(Channel {
                    offset: c,
                    demods: FmDemods::new(self.rate, c, &self.demod)?,
                    snr_db: Some(snr),
                    born_s: self.time_s,
                }),
            }
        }
        self.chans = keep;
        self.carriers = found.iter().map(|&(c, s)| (c - self.dial, s)).collect();
        Ok(())
    }
}

sdrkit_core::choice_enum! {
    /// How the receiver finds the carriers.
    pub enum Search {
        /// One channel at the dial.
        Table = ("table", "Table (the dial)"),
        /// The narrow carriers found in the input.
        Smart = ("smart", "Smart (blind search)"),
    }
}

/// How the receiver finds the carriers.
pub const SEARCH: Setting<field::Choice<Search>> =
    Setting::choice("search", "Search", Search::Table);
/// The length of a CCSDS frame.
pub const CCSDS_LENGTH: Setting<field::I64> =
    Setting::new("ccsds_length", "CCSDS code length", "B", 255).range(33.0, 255.0, 1.0);
/// The most carriers the smart search follows.
pub const MAX_CHANNELS: Setting<field::I64> =
    Setting::new("smart.max_channels", "Carriers followed", "", 4).range(1.0, 16.0, 1.0);
/// The SNR a carrier needs for the smart search.
pub const MIN_SNR_DB: Setting<field::F64> =
    Setting::new("smart.min_snr_db", "Carrier SNR", "dB", 8.0).range(-10.0, 60.0, 0.5);

sdrkit_core::choice_enum! {
    /// A Reed-Solomon symbol basis.
    pub enum RsBasis {
        /// The CCSDS dual basis.
        Dual = ("dual", "Dual"),
        /// The conventional (polynomial) basis.
        Conventional = ("conventional", "Conventional"),
    }
}

impl RsBasis {
    fn basis(self) -> Basis {
        match self {
            RsBasis::Dual => Basis::Dual,
            RsBasis::Conventional => Basis::Conventional,
        }
    }
    fn of(b: Basis) -> RsBasis {
        match b {
            Basis::Dual => RsBasis::Dual,
            Basis::Conventional => RsBasis::Conventional,
        }
    }
}

sdrkit_core::choice_enum! {
    /// The U482C audio bit rate.
    pub enum AfskBitrate {
        /// 1200 bit/s.
        B1200 = ("1200", "1200 bit/s"),
        /// 2400 bit/s.
        B2400 = ("2400", "2400 bit/s"),
        /// 4800 bit/s.
        B4800 = ("4800", "4800 bit/s"),
    }
}

impl AfskBitrate {
    fn rate(self) -> u32 {
        match self {
            AfskBitrate::B1200 => 1200,
            AfskBitrate::B2400 => 2400,
            AfskBitrate::B4800 => 4800,
        }
    }
}

sdrkit_core::choice_enum! {
    /// The Geoscan frame length after the sync word.
    pub enum GeoscanLength {
        /// 66 bytes.
        L66 = ("66", "66 bytes"),
        /// 74 bytes.
        L74 = ("74", "74 bytes"),
    }
}

/// Extra FSK slicers.
pub const EXTRA_BITRATES: Setting<field::Text> =
    Setting::new("extra_bitrates", "Extra bit rates", "bit/s", "")
        .help("Up to four bit rates of 300 to 20000 bit/s, separated by commas: one more FSK slicer each, for every framing.");
/// The AX.25 receivers on the extra slicers.
pub const EXTRA_AX25: Setting<field::Choice<ExtraAx25>> =
    Setting::choice("extra_ax25", "AX.25 on the extra slicers", ExtraAx25::Off);
/// The U482C audio carrier.
pub const AFSK_CARRIER_HZ: Setting<field::F64> =
    Setting::new("afsk_carrier_hz", "U482C audio carrier", "Hz", 3600.0)
        .range(0.0, 8000.0, 1.0)
        .help("0 turns the audio FSK path off; else 500 to 8000 Hz.");
/// The U482C audio bit rate.
pub const AFSK_BITRATE: Setting<field::Choice<AfskBitrate>> =
    Setting::choice("afsk_bitrate", "U482C audio bit rate", AfskBitrate::B4800);
/// The Geoscan frame length.
pub const GEOSCAN_LENGTH: Setting<field::Choice<GeoscanLength>> =
    Setting::choice("geoscan_length", "Geoscan frame bytes", GeoscanLength::L66);
/// The basis of the CCSDS RS framing and of Erminaz.
pub const CCSDS_BASIS: Setting<field::Choice<RsBasis>> =
    Setting::choice("ccsds_basis", "CCSDS RS basis", RsBasis::Dual);
/// The data bytes of a CCSDS concatenated frame.
pub const CONCAT_FRAME_BYTES: Setting<field::I64> =
    Setting::new("concat.frame_bytes", "CCSDS concatenated frame size", "B", 223)
        .range(1.0, 1784.0, 1.0)
        .help("Data bytes, check bytes not included: a multiple of the interleave, at most 223 for each code word.");
/// The interleave of a CCSDS concatenated frame.
pub const CONCAT_INTERLEAVE: Setting<field::I64> =
    Setting::new("concat.interleave", "Interleave", "", 1).range(1.0, 8.0, 1.0);
/// The basis of a CCSDS concatenated frame.
pub const CONCAT_BASIS: Setting<field::Choice<RsBasis>> =
    Setting::choice("concat.basis", "Basis", RsBasis::Dual);
/// The K = 7 code of a CCSDS concatenated link.
pub const CONCAT_CODE: Setting<field::Choice<ConcatCode>> =
    Setting::choice("concat.code", "Code", ConcatCode::Ccsds);
/// NRZ-M before the code.
pub const CONCAT_NRZM: Setting<field::Bool> = Setting::new("concat.nrzm", "NRZ-M", "", false);
/// The randomiser of a CCSDS concatenated link.
pub const CONCAT_RANDOMISER: Setting<field::Bool> =
    Setting::new("concat.randomiser", "Randomiser", "", true);
/// The Mobitex variant of the Mobitex-NX sync word.
pub const MOBITEX_VARIANT: Setting<field::Choice<MobitexVariant>> =
    Setting::choice("mobitex_variant", "Mobitex variant", MobitexVariant::Nx);

/// Reads the extra bit rates: up to four numbers of 300 to 20000 separated by commas or spaces. The position of the first
/// that does not read on failure.
pub fn parse_bitrates(t: &str) -> Result<Vec<u32>, usize> {
    let mut out = Vec::new();
    let mut at = 0;
    for part in t.split([',', ' ', ';']) {
        let start = at;
        at += part.len() + 1;
        if part.trim().is_empty() {
            continue;
        }
        let v: u32 = part.trim().parse().map_err(|_| start)?;
        if !(300..=20_000).contains(&v) || out.len() == 4 {
            return Err(start);
        }
        out.push(v);
    }
    Ok(out)
}

sdrkit_core::settings! {
    /// The settings of [`CubesatRx`].
    pub enum CubesatRxSetting {
        /// Search.
        Search = SEARCH,
        /// CCSDS code length.
        CcsdsLength = CCSDS_LENGTH,
        /// Carriers followed.
        MaxChannels = MAX_CHANNELS,
        /// Carrier SNR.
        MinSnrDb = MIN_SNR_DB,
        /// Extra bit rates.
        ExtraBitrates = EXTRA_BITRATES,
        /// AX.25 on the extra slicers.
        ExtraAx25 = EXTRA_AX25,
        /// U482C audio carrier.
        AfskCarrierHz = AFSK_CARRIER_HZ,
        /// U482C audio bit rate.
        AfskBitrate = AFSK_BITRATE,
        /// Geoscan frame bytes.
        GeoscanLength = GEOSCAN_LENGTH,
        /// CCSDS RS basis.
        CcsdsBasis = CCSDS_BASIS,
        /// CCSDS concatenated frame size.
        ConcatFrameBytes = CONCAT_FRAME_BYTES,
        /// Interleave.
        ConcatInterleave = CONCAT_INTERLEAVE,
        /// Basis.
        ConcatBasis = CONCAT_BASIS,
        /// Code.
        ConcatCode = CONCAT_CODE,
        /// NRZ-M.
        ConcatNrzm = CONCAT_NRZM,
        /// Randomiser.
        ConcatRandomiser = CONCAT_RANDOMISER,
        /// Mobitex variant.
        MobitexVariant = MOBITEX_VARIANT,
    }
}

impl Decoder for CubesatRx {
    type Input = Complex32;
    fn name(&self) -> &'static str {
        "satcom.cubesat_receiver"
    }
    fn outputs(&self) -> Vec<Port> {
        vec![Port::typed::<CsItem>("out").same_format_as(0)]
    }
    fn settings(&self) -> Arc<Schema> {
        CubesatRxSetting::schema()
    }
    fn values(&self) -> Values {
        let search = if self.smart {
            Search::Smart
        } else {
            Search::Table
        };
        let d = &self.demod;
        let afsk = match d.afsk_bitrate {
            1200 => AfskBitrate::B1200,
            2400 => AfskBitrate::B2400,
            _ => AfskBitrate::B4800,
        };
        let geoscan = if d.geoscan_len == 74 {
            GeoscanLength::L74
        } else {
            GeoscanLength::L66
        };
        values_of([
            (CubesatRxSetting::Search, Value::Choice(search.index())),
            (
                CubesatRxSetting::CcsdsLength,
                Value::I64(d.ccsds_len as i64),
            ),
            (
                CubesatRxSetting::MaxChannels,
                Value::I64(self.max_channels as i64),
            ),
            (CubesatRxSetting::MinSnrDb, Value::F64(self.min_snr_db)),
            (
                CubesatRxSetting::ExtraBitrates,
                Value::Text(self.extra_text.as_str().into()),
            ),
            (
                CubesatRxSetting::ExtraAx25,
                Value::Choice(d.extra_ax25.index()),
            ),
            (
                CubesatRxSetting::AfskCarrierHz,
                Value::F64(d.afsk_carrier_hz),
            ),
            (CubesatRxSetting::AfskBitrate, Value::Choice(afsk.index())),
            (
                CubesatRxSetting::GeoscanLength,
                Value::Choice(geoscan.index()),
            ),
            (
                CubesatRxSetting::CcsdsBasis,
                Value::Choice(RsBasis::of(d.ccsds_basis).index()),
            ),
            (
                CubesatRxSetting::ConcatFrameBytes,
                Value::I64(d.concat.frame_bytes as i64),
            ),
            (
                CubesatRxSetting::ConcatInterleave,
                Value::I64(d.concat.interleave as i64),
            ),
            (
                CubesatRxSetting::ConcatBasis,
                Value::Choice(RsBasis::of(d.concat.basis).index()),
            ),
            (
                CubesatRxSetting::ConcatCode,
                Value::Choice(d.concat.code.index()),
            ),
            (CubesatRxSetting::ConcatNrzm, Value::Bool(d.concat.nrzm)),
            (
                CubesatRxSetting::ConcatRandomiser,
                Value::Bool(d.concat.randomiser),
            ),
            (
                CubesatRxSetting::MobitexVariant,
                Value::Choice(d.mobitex_variant.index()),
            ),
        ])
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<(), SettingError> {
        match CubesatRxSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            CubesatRxSetting::Search => self.smart = SEARCH.read(id, v)? == Search::Smart,
            CubesatRxSetting::CcsdsLength => {
                self.demod.ccsds_len = usize::try_from(CCSDS_LENGTH.read(id, v)?)
                    .map_err(|_| SettingError::NotPermitted { id })?;
            }
            CubesatRxSetting::ExtraBitrates => {
                let t = EXTRA_BITRATES.read(id, v)?;
                self.demod.extra_bitrates =
                    parse_bitrates(t).map_err(|at| SettingError::Malformed { id, at: Some(at) })?;
                self.extra_text = t.to_string();
            }
            CubesatRxSetting::ExtraAx25 => self.demod.extra_ax25 = EXTRA_AX25.read(id, v)?,
            CubesatRxSetting::AfskCarrierHz => {
                let hz = AFSK_CARRIER_HZ.read(id, v)?;
                if hz != 0.0 && hz < 500.0 {
                    return Err(SettingError::NotPermitted { id });
                }
                self.demod.afsk_carrier_hz = hz;
            }
            CubesatRxSetting::AfskBitrate => {
                self.demod.afsk_bitrate = AFSK_BITRATE.read(id, v)?.rate();
            }
            CubesatRxSetting::GeoscanLength => {
                self.demod.geoscan_len = match GEOSCAN_LENGTH.read(id, v)? {
                    GeoscanLength::L66 => 66,
                    GeoscanLength::L74 => 74,
                };
            }
            CubesatRxSetting::CcsdsBasis => {
                self.demod.ccsds_basis = CCSDS_BASIS.read(id, v)?.basis();
            }
            CubesatRxSetting::ConcatFrameBytes => {
                self.demod.concat.frame_bytes = usize::try_from(CONCAT_FRAME_BYTES.read(id, v)?)
                    .map_err(|_| SettingError::NotPermitted { id })?;
            }
            CubesatRxSetting::ConcatInterleave => {
                self.demod.concat.interleave = usize::try_from(CONCAT_INTERLEAVE.read(id, v)?)
                    .map_err(|_| SettingError::NotPermitted { id })?;
            }
            CubesatRxSetting::ConcatBasis => {
                self.demod.concat.basis = CONCAT_BASIS.read(id, v)?.basis();
            }
            CubesatRxSetting::ConcatCode => self.demod.concat.code = CONCAT_CODE.read(id, v)?,
            CubesatRxSetting::ConcatNrzm => self.demod.concat.nrzm = CONCAT_NRZM.read(id, v)?,
            CubesatRxSetting::ConcatRandomiser => {
                self.demod.concat.randomiser = CONCAT_RANDOMISER.read(id, v)?;
            }
            CubesatRxSetting::MobitexVariant => {
                self.demod.mobitex_variant = MOBITEX_VARIANT.read(id, v)?;
            }
            CubesatRxSetting::MaxChannels => {
                self.max_channels = usize::try_from(MAX_CHANNELS.read(id, v)?)
                    .map_err(|_| SettingError::NotPermitted { id })?;
            }
            CubesatRxSetting::MinSnrDb => self.min_snr_db = MIN_SNR_DB.read(id, v)?,
        }
        if self.rate > 0.0 {
            self.build().map_err(|e| SettingError::Config {
                id,
                source: Box::new(e),
            })?;
        }
        Ok(())
    }
    fn configure(&mut self, f: &Format) -> Result<Vec<Format>, ConfigError> {
        let need = min_rate(self.demod.submode);
        if f.rate < need {
            return Err(ConfigError::RateTooLow { need, got: f.rate });
        }
        self.rate = f.rate;
        self.dial = f.signal_offset_hz();
        self.time_s = 0.0;
        self.build()?;
        Ok(vec![f.with_kind(Kind::typed::<CsItem>())])
    }
    fn retune(&mut self, f: &Format) {
        self.dial = f.signal_offset_hz();
        if self.build().is_err() {
            self.chans.clear();
        }
    }
    fn process(&mut self, iq: &[Complex32], first: u64, out: &mut Outbox) {
        if let Some(sa) = self.scan.as_mut() {
            sa.push(iq);
        }
        let items = out.items::<CsItem>(0);
        for ch in &mut self.chans {
            self.found.clear();
            ch.demods.process(iq, &mut self.found);
            for f in self.found.drain(..) {
                items.push(Timed {
                    time: ch.born_s + f.at as f64 / demod::RATE,
                    value: CsItem::Raw {
                        frame: f,
                        carrier_hz: ch.offset + ch.demods.offset_hz,
                        snr_db: ch.snr_db,
                    },
                });
            }
        }
        self.time_s = (first + iq.len() as u64) as f64 / self.rate;
    }
    fn tick(&mut self, out: &mut Outbox) {
        if self.scan.as_ref().is_some_and(|s| s.frames() >= 8) {
            if self.rescan().is_err() {
                self.chans.clear();
            }
            if let Some(s) = self.scan.as_mut() {
                s.reset();
            }
        }
        let carrier = if self.smart {
            !self.carriers.is_empty()
        } else {
            self.chans.first().is_some_and(|c| c.demods.level > 1e-6)
        };
        let offset_hz = self
            .chans
            .first()
            .map_or(0.0, |c| c.offset - self.dial + c.demods.offset_hz);
        self.counts.clear();
        for c in &self.chans {
            c.demods.counts(&mut self.counts);
        }
        out.items::<CsItem>(0).push(Timed {
            time: self.time_s,
            value: CsItem::Status {
                carrier,
                offset_hz,
                carriers: self.carriers.clone(),
                counts: self.counts.clone(),
            },
        });
    }
}

/// The satellite table, "name|norad|MHz|framing|baud|call;...".
pub const SATELLITES: Setting<field::Text> =
    Setting::new("satellites", "Satellite table", "", DEFAULT_TABLE)
        .help("name|norad|MHz|framing|baud|call;...");
/// The satellite the pass planner follows, "name|norad".
pub const TRACKED: Setting<field::Text> = Setting::new("tracked", "Satellite followed", "", "")
    .help("name|norad of the satellite the pass planner follows (from the host).");

sdrkit_core::settings! {
    /// The settings of [`NamingStage`].
    pub enum NamingSetting {
        /// Satellite table.
        Satellites = SATELLITES,
        /// Satellite followed.
        Tracked = TRACKED,
    }
}

/// The naming decoder (`satcom.cubesat_naming`): reads each frame's fields and names its satellite.
pub struct NamingStage {
    table_text: String,
    tracked_text: String,
    table: Vec<SatEntry>,
    tracked: Option<(String, u32)>,
    dial_hz: f64,
    centre: f64,
}

impl Default for NamingStage {
    fn default() -> Self {
        NamingStage {
            table_text: DEFAULT_TABLE.into(),
            tracked_text: String::new(),
            table: parse_table(DEFAULT_TABLE).unwrap_or_default(),
            tracked: None,
            dial_hz: 0.0,
            centre: 0.0,
        }
    }
}

impl NamingStage {
    /// The fields and the satellite of a frame.
    pub fn name_frame(
        &self,
        frame: RawFrame,
        carrier_hz: f64,
        time: f64,
        snr_db: Option<f64>,
    ) -> Named {
        let mut r = Named {
            time,
            freq_hz: self.centre + carrier_hz,
            snr_db,
            ..Default::default()
        };
        let mut call = String::new();
        let ax25 = match frame.framing {
            f if f.is_ax25() => Some(&frame.data[..]),
            Framing::Usp => Some(&frame.data[..]),
            Framing::Geoscan => frame.data.get(..frame.data.len().saturating_sub(2)),
            _ => None,
        };
        if let Some(bytes) = ax25 {
            if let Ok(a) = sdrkit_proto_link::ax25::Frame::parse(bytes) {
                r.from = a.source().to_string();
                r.to = a.destination().to_string();
                r.path = a
                    .repeaters()
                    .iter()
                    .map(|p| p.to_string())
                    .collect::<Vec<_>>()
                    .join(",");
                r.frame_type = a.control.name().into();
                r.pid = a.pid;
                r.payload = a.info.to_vec();
                call = a.source().call.as_str().to_string();
            }
            if frame.framing == Framing::Geoscan {
                r.payload = frame.data.clone();
            }
        } else if frame.framing.is_ax100() || frame.framing == Framing::U482c {
            if let Ok(h) = sdrkit_proto_link::csp::Header::parse(&frame.data) {
                r.csp = framing::csp_words(&h);
            }
            r.payload = frame.data.get(4..).unwrap_or(&[]).to_vec();
        } else {
            r.payload = frame.data.clone();
            if let Some(c) = &frame.callsign {
                r.from = c.clone();
                call = c.clone();
            }
        }
        r.text = framing::text_beacon(&r.payload);
        // The callsign names the frame, else the satellite followed, else the dial with a matching framing.
        if let Some(e) = self
            .table
            .iter()
            .find(|e| !e.call.is_empty() && e.call.eq_ignore_ascii_case(&call))
        {
            r.satellite = e.name.clone();
            r.norad = e.norad;
        } else if let Some((n, id)) = &self.tracked {
            r.satellite = n.clone();
            r.norad = *id;
        } else if let Some(e) = self.table.iter().find(|e| {
            (e.mhz * 1e6 - self.dial_hz).abs() < FOLLOW_HZ
                && e.framing.is_none_or(|f| f == frame.framing)
        }) {
            r.satellite = e.name.clone();
            r.norad = e.norad;
            r.guess = true;
        }
        r.raw = Some(frame);
        r
    }
}

impl Decoder for NamingStage {
    type Input = Timed<CsItem>;
    fn name(&self) -> &'static str {
        "satcom.cubesat_naming"
    }
    fn outputs(&self) -> Vec<Port> {
        vec![Port::typed::<CsItem>("out").same_format_as(0)]
    }
    fn settings(&self) -> Arc<Schema> {
        NamingSetting::schema()
    }
    fn values(&self) -> Values {
        values_of([
            (
                NamingSetting::Satellites,
                Value::Text(self.table_text.as_str().into()),
            ),
            (
                NamingSetting::Tracked,
                Value::Text(self.tracked_text.as_str().into()),
            ),
        ])
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<(), SettingError> {
        match NamingSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            NamingSetting::Satellites => {
                let t = SATELLITES.read(id, v)?;
                self.table =
                    parse_table(t).map_err(|at| SettingError::Malformed { id, at: Some(at) })?;
                self.table_text = t.to_string();
            }
            NamingSetting::Tracked => {
                let t = TRACKED.read(id, v)?;
                self.tracked =
                    parse_tracked(t).map_err(|at| SettingError::Malformed { id, at: Some(at) })?;
                self.tracked_text = t.to_string();
            }
        }
        Ok(())
    }
    fn configure(&mut self, f: &Format) -> Result<Vec<Format>, ConfigError> {
        self.dial_hz = f.tuned_hz;
        self.centre = f.centre_hz;
        Ok(vec![f.with_kind(Kind::typed::<CsItem>())])
    }
    fn retune(&mut self, f: &Format) {
        self.dial_hz = f.tuned_hz;
        self.centre = f.centre_hz;
    }
    fn process(&mut self, x: &[Timed<CsItem>], _first: u64, out: &mut Outbox) {
        let items = out.items::<CsItem>(0);
        for it in x {
            match &it.value {
                CsItem::Raw {
                    frame,
                    carrier_hz,
                    snr_db,
                } => items.push(Timed {
                    time: it.time,
                    value: CsItem::Named(Box::new(self.name_frame(
                        frame.clone(),
                        *carrier_hz,
                        it.time,
                        *snr_db,
                    ))),
                }),
                _ => items.push(it.clone()),
            }
        }
    }
}

/// The settings of the receiver, the naming, the display, the sender and the test signal, in that order.
pub(crate) const SETTINGS: &[&dyn DeclareSetting] = &[
    &SEARCH,
    &CCSDS_LENGTH,
    &MAX_CHANNELS,
    &MIN_SNR_DB,
    &EXTRA_BITRATES,
    &EXTRA_AX25,
    &AFSK_CARRIER_HZ,
    &AFSK_BITRATE,
    &GEOSCAN_LENGTH,
    &CCSDS_BASIS,
    &CONCAT_FRAME_BYTES,
    &CONCAT_INTERLEAVE,
    &CONCAT_BASIS,
    &CONCAT_CODE,
    &CONCAT_NRZM,
    &CONCAT_RANDOMISER,
    &MOBITEX_VARIANT,
    &SATELLITES,
    &TRACKED,
    &display::KISS,
    &display::FRAMING_FILTER,
    &tx::TX_RATE,
    &tx::TX_DELAY,
    &tx::TX_LEVEL,
    &gen::GEN_FRAMING,
];

/// Every setting of the stack. Noise and the carrier offset of the test signal come from the standard `gen_*` settings.
pub fn schema() -> Arc<Schema> {
    static SCHEMA: OnceLock<Arc<Schema>> = OnceLock::new();
    SCHEMA
        .get_or_init(|| Arc::new(Schema::of(SETTINGS)))
        .clone()
}

/// The stack's graph: the demodulators, the naming and the display; the AX.25 sender takes the same input and writes into
/// the transmit sink.
///
/// # Errors
///
/// A [`GraphError`] if a setting of `cfg` is refused.
pub fn receiver(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    let sub = Submode::of(cfg.submode.as_deref()).ok_or_else(|| GraphError::Config {
        component: "receiver".into(),
        error: crate::stage_error(UnknownSubmode),
    })?;
    let mut b = GraphBuilder::new()
        .add("receiver", DecoderComponent::new(CubesatRx::new(sub)))
        .add("naming", DecoderComponent::new(NamingStage::default()))
        .add("display", DecoderComponent::new(CsDisplay::new(sub)))
        .add("sender", tx::CubesatSender::default())
        .add("tx", sdrkit_graph::components::TxSink::default())
        .input("receiver.in")
        .input("sender.in")
        .connect("sender.out", "tx.in")
        .connect("receiver.out", "naming.in")
        .connect("naming.out", "display.in")
        .output("display.events")
        .output("sender.events")
        .status_from("display")
        .clock(REPORT_S);
    for (id, schema) in [
        ("receiver", CubesatRx::default().settings()),
        ("naming", NamingStage::default().settings()),
        ("display", CsDisplay::default().settings()),
    ] {
        for s in schema.settings() {
            b = b.alias(&s.field.key, &format!("{id}.{}", s.field.key));
        }
    }
    for key in [tx::TX_RATE.key(), tx::TX_DELAY.key(), tx::TX_LEVEL.key()] {
        b = b.alias(key, &format!("sender.{key}"));
    }
    let g = b.catalog(&Catalog::of(cfg)?).build()?;
    StackGraph::of(crate::typed::shape(ID), g)
        .min_rate(
            min_rate(sub),
            if sub == Submode::All {
                "CubeSat telemetry needs a sample rate of at least 1000 ksps"
            } else {
                "A CubeSat submode needs a sample rate of at least 130 ksps"
            },
        )
        .default_centre(DESCRIPTOR.tuning.default_hz + DESCRIPTOR.tuning.tune_offset_hz)
        .command("clear", sdrkit_graph::CommandTarget::every("clear"))
        .command(
            crate::typed::CUBESAT_SEND.command.key,
            sdrkit_graph::CommandTarget::of("sender", crate::typed::CUBESAT_SEND.command.key),
        )
        .command(
            crate::typed::CUBESAT_HALT.key,
            sdrkit_graph::CommandTarget::of("sender", crate::typed::CUBESAT_HALT.key),
        )
        .with_settings(&cfg.settings)
        .build()
}

/// A submode the stack does not have.
#[derive(Debug)]
struct UnknownSubmode;

impl std::fmt::Display for UnknownSubmode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("the CubeSat stack has no such submode")
    }
}

impl std::error::Error for UnknownSubmode {}

/// The lowest sample rate of `sub`.
pub fn min_rate(sub: Submode) -> f64 {
    if sub == Submode::All {
        MIN_RATE
    } else {
        MIN_RATE_NAMED
    }
}

fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    crate::stack_receiver(receiver(cfg))
}

fn make_generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(Box::new(gen::CubesatGenerator::from_config(cfg)))
}

/// What the stack publishes.
pub fn declaration() -> ResultsDecl {
    crate::typed::shape(ID).results_decl()
}

/// Adds the components of the stack.
pub fn register(r: &mut Registry) {
    r.add("satcom.cubesat_receiver", || {
        Box::new(DecoderComponent::new(CubesatRx::default()))
    });
    r.add("satcom.cubesat_naming", || {
        Box::new(DecoderComponent::new(NamingStage::default()))
    });
    r.add("satcom.cubesat_display", || {
        Box::new(DecoderComponent::new(CsDisplay::default()))
    });
}

/// The verification of the stack: the test signal's cycle decodes.
pub static HOOKS: VerifyHooks = VerifyHooks {
    cases,
    vector_settings: None,
    check_vector: None,
};

fn cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let mut c = VerifyCase::new("test signal, every framing", o, 700.0);
    c.seconds = c.seconds.max(4.0);
    c.expect_audio = Some(false);
    c.check = Some(check_frames);
    let mut out = vec![c];
    for s in &SUBMODES {
        let mut c = VerifyCase::new(s.name, o, 700.0);
        c.submode = Some(s.id.into());
        c.seconds = c.seconds.max(3.0);
        c.expect_audio = Some(false);
        c.check = Some(check_one_frame);
        out.push(c);
    }
    out
}

fn check_one_frame(r: &sdrkit_core::Results) -> Vec<CheckLine> {
    let good = crate::field(r, "frames_ok");
    vec![CheckLine::new(
        "frames_ok",
        good >= 1.0,
        format!("{good} frames decoded"),
    )]
}

/// The named submodes. The default frequency of each is the test-signal dial, not a satellite downlink.
pub static SUBMODES: [SubMode; 9] = [
    SubMode {
        id: "geoscan",
        name: "Geoscan",
        default_hz: TEST_SIGNAL_HZ,
        ..SubMode::DEFAULT
    },
    SubMode {
        id: "lucky7",
        name: "Lucky-7",
        default_hz: TEST_SIGNAL_HZ,
        ..SubMode::DEFAULT
    },
    SubMode {
        id: "spino",
        name: "Spino",
        default_hz: TEST_SIGNAL_HZ,
        ..SubMode::DEFAULT
    },
    SubMode {
        id: "erminaz",
        name: "Erminaz",
        default_hz: TEST_SIGNAL_HZ,
        ..SubMode::DEFAULT
    },
    SubMode {
        id: "usp",
        name: "USP",
        default_hz: TEST_SIGNAL_HZ,
        ..SubMode::DEFAULT
    },
    SubMode {
        id: "ngham",
        name: "NGHam",
        default_hz: TEST_SIGNAL_HZ,
        ..SubMode::DEFAULT
    },
    SubMode {
        id: "u482c",
        name: "U482C",
        default_hz: TEST_SIGNAL_HZ,
        ..SubMode::DEFAULT
    },
    SubMode {
        id: "mobitex",
        name: "Mobitex",
        default_hz: TEST_SIGNAL_HZ,
        ..SubMode::DEFAULT
    },
    SubMode {
        id: "ccsds_concat",
        name: "CCSDS concatenated",
        default_hz: TEST_SIGNAL_HZ,
        ..SubMode::DEFAULT
    },
];

/// The benchmarks of the receiver: every framing at once on the test signal at 2 Msps.
pub fn benchmarks() -> Vec<sdrkit_core::Benchmark> {
    vec![sdrkit_core::Benchmark {
        name: "cubesat.receiver_2m",
        run: |s| crate::bench_receiver(s, &DESCRIPTOR, None, 4.0),
    }]
}

fn check_frames(r: &sdrkit_core::Results) -> Vec<CheckLine> {
    let good = crate::field(r, "frames_ok");
    vec![CheckLine::new(
        "frames_ok",
        good >= 3.0,
        format!("{good} frames decoded"),
    )]
}

/// The descriptor of the stack.
pub static DESCRIPTOR: StackDescriptor = StackDescriptor {
    id: ID,
    name: NAME,
    group: "Satellite",
    blurb: "Amateur and small-satellite packet telemetry: AX.25 (AFSK, G3RUH), AX100 Mode 5, CCSDS, and Geoscan, Lucky-7, Spino, Erminaz, USP, NGHam, U482C, Mobitex and CCSDS concatenated frames. Sends AX.25 UI frames in AFSK 1200 and G3RUH 9600 for an amateur satellite uplink.",
    tuning: Tuning {
        kind: Some(sdrkit_core::TuningKind::Channels),
        min_hz: 130e6,
        max_hz: 1000e6,
        default_hz: TEST_SIGNAL_HZ,
        rate_hz: 2e6,
        min_rate_hz: MIN_RATE,
        bandwidth_hz: 30e3,
        tune_offset_hz: 50e3,
        carrier_at_centre: false,
        not_circular: false,
        radio_filter_hz: Some(1.75e6),
        no_offset_tuning: false,
        ..Tuning::DEFAULT
    },
    submodes: &SUBMODES,
    settings: schema,
    make_receiver,
    make_generator: Some(make_generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[SignalType::PacketAfsk1200],
    auto_gain: AutoGain::Radio,
    verify: Some(&HOOKS),
    benchmarks: None,
};
