//! The Wi-Fi networks monitor (`wifi`): the beacons, probe responses and FILS Discovery frames that access points broadcast on
//! 2.4, 5 and 6 GHz, heard with the DSSS and non-HT OFDM receivers, and a network table.
//! Standard: IEEE Std 802.11-2020 clauses 9, 16 and 17; IEEE Std 802.11ax-2021; IEEE Std 802.11be-2024.

pub mod frames;
pub mod gen;
pub mod mac;
pub mod plan;
pub mod table;

use sdrkit_algo_dsp::meter::IntervalLevel;

use crate::testgen::{gen_setting, route_to, GEN_SECTION};
use plan::{centre_mhz, channel_at, channels_in_capture, Band};
use sdrkit_algo_dsp::channel::{ChannelConfig, Channelizer};
use sdrkit_core::publish::field;
use sdrkit_core::SignalType;
use sdrkit_core::{
    AutoGain, Complex32, ConfigError, GenConfig, Generator, LockProfile, Receiver, Setting,
    SettingSet, StackConfig, StackDescriptor, Tuning, Values,
};
use sdrkit_graph::{
    no_values, Catalog, Decoder, DecoderComponent, Format, Graph, GraphBuilder, GraphError, Item,
    Kind, Outbox, Port, Registry, StackGraph, Timed,
};
use sdrkit_proto_wlan_phy::{dsss, ofdm};
use std::sync::Arc;

/// The stack id.
pub const ID: &str = "wifi";
/// The lowest input rate, Hz.
pub const MIN_RATE_HZ: f64 = 20e6;
/// From this rate a 5 or 6 GHz capture is split into its 20 MHz channels.
pub const WIDE_RATE_HZ: f64 = 43.5e6;
/// The rate both receivers run at, Hz.
pub const CHAIN_RATE_HZ: f64 = 20e6;
/// The width each channel filter keeps flat, Hz (the OFDM tones span 16.6 MHz).
const CHAIN_WIDTH_HZ: f64 = 17e6;

sdrkit_core::choice_enum! {
    /// The PHY a frame came on.
    #[derive(serde::Serialize, serde::Deserialize)]
    pub enum PhyKind {
        /// DSSS (clause 16).
        Dsss = ("dsss", "DSSS"),
        /// Non-HT OFDM (clause 17).
        Ofdm = ("ofdm", "OFDM"),
    }
}

impl PhyKind {
    /// "DSSS" or "OFDM".
    pub fn name(self) -> &'static str {
        match self {
            PhyKind::Dsss => "DSSS",
            PhyKind::Ofdm => "OFDM",
        }
    }
}

/// An MPDU whose FCS passed (the `wifi.mpdu` stream).
#[derive(Clone, Debug, PartialEq)]
pub struct WifiMpdu {
    /// The bytes, FCS removed.
    pub bytes: Vec<u8>,
    /// The PHY.
    pub phy: PhyKind,
    /// The data rate, Mbit/s.
    pub rate_mbps: f64,
    /// The level of the preamble, dBFS.
    pub level_dbfs: f64,
    /// Carrier offset, Hz.
    pub offset_hz: f64,
    /// Signal-to-noise ratio, dB.
    pub snr_db: Option<f64>,
    /// Signal time, s.
    pub time_s: f64,
    /// The channel it was heard on (0: not known).
    pub channel: u32,
    /// The band it was heard on.
    pub band: Option<Band>,
    /// A DSSS frame with the short preamble.
    pub short_preamble: bool,
}

impl Item for WifiMpdu {
    const KIND: &'static str = "wifi.mpdu";
}

/// The DSSS receiver's counters, summed over every receiver the stack has run.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DsssCounters {
    /// SFDs found.
    pub sfds: u64,
    /// Headers with a good CRC.
    pub headers_ok: u64,
    /// Headers with a bad CRC or a length that does not fit.
    pub headers_bad: u64,
    /// Good headers at a CCK rate (5.5 or 11 Mbit/s, not decoded).
    pub other_rate: u64,
    /// PSDUs with a good FCS.
    pub frames: u64,
    /// PSDUs with a bad FCS.
    pub bad_fcs: u64,
}

impl DsssCounters {
    fn add(&mut self, s: dsss::DsssStats) {
        self.sfds += s.sfds;
        self.headers_ok += s.headers_ok;
        self.headers_bad += s.headers_bad;
        self.other_rate += s.other_rate;
        self.frames += s.frames;
        self.bad_fcs += s.bad_fcs;
    }
    /// The counts since `base`.
    pub fn since(self, base: DsssCounters) -> DsssCounters {
        DsssCounters {
            sfds: self.sfds.saturating_sub(base.sfds),
            headers_ok: self.headers_ok.saturating_sub(base.headers_ok),
            headers_bad: self.headers_bad.saturating_sub(base.headers_bad),
            other_rate: self.other_rate.saturating_sub(base.other_rate),
            frames: self.frames.saturating_sub(base.frames),
            bad_fcs: self.bad_fcs.saturating_sub(base.bad_fcs),
        }
    }
}

/// The OFDM receivers' counters, summed over every chain the stack has run.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct OfdmCounters {
    /// Short training fields detected.
    pub preambles: u64,
    /// Detections with no long training field.
    pub no_sync: u64,
    /// SIGNAL fields that passed.
    pub signal_ok: u64,
    /// SIGNAL fields that failed.
    pub signal_bad: u64,
    /// PSDUs with a good FCS.
    pub frames: u64,
    /// PSDUs with a bad FCS.
    pub bad_fcs: u64,
}

impl OfdmCounters {
    fn add(&mut self, s: ofdm::OfdmStats) {
        self.preambles += s.preambles;
        self.no_sync += s.no_sync;
        self.signal_ok += s.signal_ok;
        self.signal_bad += s.signal_bad;
        self.frames += s.frames;
        self.bad_fcs += s.bad_fcs;
    }
    /// The counts since `base`.
    pub fn since(self, base: OfdmCounters) -> OfdmCounters {
        OfdmCounters {
            preambles: self.preambles.saturating_sub(base.preambles),
            no_sync: self.no_sync.saturating_sub(base.no_sync),
            signal_ok: self.signal_ok.saturating_sub(base.signal_ok),
            signal_bad: self.signal_bad.saturating_sub(base.signal_bad),
            frames: self.frames.saturating_sub(base.frames),
            bad_fcs: self.bad_fcs.saturating_sub(base.bad_fcs),
        }
    }
}

/// The receivers' counters and the input level (the `wifi.stats` stream), sent after each piece of input.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct WifiStats {
    /// Signal time at the end of the piece, s.
    pub time_s: f64,
    /// DSSS counters.
    pub dsss: DsssCounters,
    /// OFDM counters.
    pub ofdm: OfdmCounters,
    /// OFDM chains running.
    pub sub_channels: u32,
    /// The level of the input over the piece, dBFS.
    pub level_dbfs: Option<f64>,
    /// The band of the centre.
    pub band: Option<Band>,
    /// The tuned channel (0: not known).
    pub channel: u32,
    /// The offset of the tuned channel's centre from the radio centre, Hz.
    pub channel_offset_hz: f64,
    /// The input rate, Hz.
    pub rate_hz: f64,
}

impl Item for WifiStats {
    const KIND: &'static str = "wifi.stats";
}

/// The band and tuned channel of a centre and offset (`None` and 0 when the centre is not known).
pub fn tuning(centre_hz: f64, offset_hz: f64) -> (Option<Band>, u32) {
    if centre_hz <= 0.0 {
        return (None, 0);
    }
    let band = Band::of_mhz(centre_hz / 1e6);
    let ch = band
        .and_then(|b| channel_at(b, (centre_hz + offset_hz) / 1e6))
        .unwrap_or(0);
    (band, ch)
}

/// The 2.4 GHz channel whose centre is nearest `centre_hz + offset_hz` and within 12 MHz of it, with the offset of that centre
/// from `centre_hz`. `None` outside the band or for an unknown centre.
pub fn nearest_24(centre_hz: f64, offset_hz: f64) -> Option<(u32, f64)> {
    if centre_hz <= 0.0 {
        return None;
    }
    let at_mhz = (centre_hz + offset_hz) / 1e6;
    plan::all_channels(Band::G2)
        .into_iter()
        .filter_map(|c| centre_mhz(Band::G2, c).map(|f| (c, f)))
        .filter(|(_, f)| (f - at_mhz).abs() <= 12.0)
        .min_by(|a, b| (a.1 - at_mhz).abs().total_cmp(&(b.1 - at_mhz).abs()))
        .map(|(c, f)| (c, f * 1e6 - centre_hz))
}

enum Rx {
    Dsss(Box<dsss::DsssReceiver>),
    Ofdm(Box<ofdm::OfdmReceiver>),
}

/// One 20 MHz channel: moved to 0 Hz and resampled to 20 Msps, with its receiver.
struct Chain {
    ch: Channelizer,
    rx: Rx,
    buf: Vec<Complex32>,
    /// Signal time of the chain's first sample, s.
    t0: f64,
    band: Option<Band>,
    channel: u32,
}

impl Chain {
    fn new(
        rate: f64,
        offset: f64,
        rx: Rx,
        t0: f64,
        band: Option<Band>,
        channel: u32,
    ) -> Option<Chain> {
        let mut cfg = ChannelConfig::new(rate, offset, CHAIN_WIDTH_HZ, CHAIN_RATE_HZ);
        cfg.exact_rate = true;
        Some(Chain {
            ch: Channelizer::new(cfg).ok()?,
            rx,
            buf: Vec::new(),
            t0,
            band,
            channel,
        })
    }

    fn run(&mut self, x: &[Complex32], out: &mut Vec<Timed<WifiMpdu>>) {
        self.buf.clear();
        self.ch.process(x, &mut self.buf);
        let (t0, band, channel) = (self.t0, self.band, self.channel);
        let at = |n: u64| t0 + n as f64 / CHAIN_RATE_HZ;
        match &mut self.rx {
            Rx::Dsss(r) => r.process(&self.buf, |f| {
                let rate = f.header.rate.units_100k() as f64 / 10.0;
                let mut m = mpdu(
                    f.mpdu,
                    PhyKind::Dsss,
                    rate,
                    f.level_dbfs,
                    f.offset_hz,
                    f.snr_db,
                    at(f.sample),
                    channel,
                    band,
                );
                m.value.short_preamble = f.preamble == dsss::Preamble::Short;
                out.push(m);
            }),
            Rx::Ofdm(r) => r.process(&self.buf, |f| {
                let rate = f.signal.rate.mbps() as f64;
                out.push(mpdu(
                    f.mpdu,
                    PhyKind::Ofdm,
                    rate,
                    f.level_dbfs,
                    f.offset_hz,
                    f.snr_db,
                    at(f.sample),
                    channel,
                    band,
                ));
            }),
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn mpdu(
    bytes: &[u8],
    phy: PhyKind,
    rate_mbps: f64,
    level_dbfs: f64,
    offset_hz: f64,
    snr_db: f64,
    time_s: f64,
    channel: u32,
    band: Option<Band>,
) -> Timed<WifiMpdu> {
    Timed {
        time: time_s,
        value: WifiMpdu {
            bytes: bytes.to_vec(),
            phy,
            rate_mbps,
            level_dbfs: level_dbfs.max(-200.0),
            offset_hz,
            snr_db: Some(snr_db),
            time_s,
            channel,
            band,
            short_preamble: false,
        },
    }
}

/// The Wi-Fi receiver as a decoder (`wlan.wifi_rx`): the DSSS receiver when the band is 2.4 GHz or not known, and one OFDM
/// receiver for each 20 MHz channel wholly inside a wide 5 or 6 GHz capture (from 43.5 Msps), else one at the signal offset. A
/// retune rebuilds the chains and keeps the counters.
#[derive(Default)]
pub struct WifiDecoder {
    chains: Vec<Chain>,
    rate: f64,
    kept_dsss: DsssCounters,
    kept_ofdm: OfdmCounters,
    band: Option<Band>,
    channel: u32,
    channel_offset: f64,
    nearest: bool,
    input: u64,
    level: IntervalLevel,
    out: Vec<Timed<WifiMpdu>>,
}

impl WifiDecoder {
    /// A decoder that, at 2.4 GHz, runs its receivers at the channel nearest the radio centre (within 12 MHz) instead of at the
    /// signal offset.
    pub fn nearest_channel() -> WifiDecoder {
        WifiDecoder {
            nearest: true,
            ..WifiDecoder::default()
        }
    }

    fn counters(&self) -> (DsssCounters, OfdmCounters) {
        let (mut d, mut o) = (self.kept_dsss, self.kept_ofdm);
        for c in &self.chains {
            match &c.rx {
                Rx::Dsss(r) => d.add(r.stats()),
                Rx::Ofdm(r) => o.add(r.stats()),
            }
        }
        (d, o)
    }

    fn rebuild(&mut self, f: &Format) {
        let (d, o) = self.counters();
        self.kept_dsss = d;
        self.kept_ofdm = o;
        self.chains.clear();
        let rate = f.rate;
        let centre = f.centre_hz;
        let mut offset = f.signal_offset_hz();
        let (band, mut ch) = tuning(centre, offset);
        if let (true, Some((c, off))) = (self.nearest, nearest_24(centre, offset)) {
            ch = c;
            offset = off;
        }
        self.band = band;
        self.channel = ch;
        self.channel_offset = offset;
        let t0 = self.input as f64 / rate.max(1.0);
        if matches!(band, None | Some(Band::G2)) {
            if let Ok(r) = dsss::DsssReceiver::new((CHAIN_RATE_HZ / 1e6) as usize) {
                self.chains.extend(Chain::new(
                    rate,
                    offset,
                    Rx::Dsss(Box::new(r)),
                    t0,
                    band,
                    ch,
                ));
            }
        }
        let mut offsets: Vec<(f64, u32)> = Vec::new();
        if let (Some(b), true) = (band, rate >= WIDE_RATE_HZ && centre > 0.0) {
            if b != Band::G2 {
                for c in channels_in_capture(b, centre / 1e6, rate) {
                    if let Some(mhz) = centre_mhz(b, c) {
                        offsets.push((mhz * 1e6 - centre, c));
                    }
                }
            }
        }
        if offsets.is_empty() {
            offsets.push((offset, ch));
        }
        for (off, c) in offsets {
            let r = Rx::Ofdm(Box::new(ofdm::OfdmReceiver::new()));
            self.chains.extend(Chain::new(rate, off, r, t0, band, c));
        }
    }
}

impl Decoder for WifiDecoder {
    type Input = Complex32;
    fn name(&self) -> &'static str {
        "wlan.wifi_rx"
    }
    fn outputs(&self) -> Vec<Port> {
        vec![
            Port::typed::<WifiMpdu>("mpdus"),
            Port::typed::<WifiStats>("stats"),
        ]
    }
    fn values(&self) -> Values {
        no_values()
    }
    fn configure(&mut self, input: &Format) -> Result<Vec<Format>, ConfigError> {
        if input.rate < MIN_RATE_HZ * 0.999_999 {
            return Err(ConfigError::RateTooLow {
                need: MIN_RATE_HZ,
                got: input.rate,
            });
        }
        self.rate = input.rate;
        self.input = 0;
        self.level = IntervalLevel::default();
        self.rebuild(input);
        Ok(vec![
            Format::new(Kind::typed::<WifiMpdu>(), 0.0),
            Format::new(Kind::typed::<WifiStats>(), 0.0),
        ])
    }
    fn retune(&mut self, input: &Format) {
        self.rebuild(input);
    }
    fn process(&mut self, x: &[Complex32], first: u64, out: &mut Outbox) {
        self.input = first + x.len() as u64;
        self.level.push(x);
        self.out.clear();
        for c in &mut self.chains {
            c.run(x, &mut self.out);
        }
        self.out.sort_by(|a, b| a.time.total_cmp(&b.time));
        out.items::<WifiMpdu>(0).append(&mut self.out);
        let (dsss, ofdm) = self.counters();
        let time_s = self.input as f64 / self.rate.max(1.0);
        let stats = WifiStats {
            time_s,
            dsss,
            ofdm,
            sub_channels: self
                .chains
                .iter()
                .filter(|c| matches!(c.rx, Rx::Ofdm(_)))
                .count() as u32,
            level_dbfs: self.level.take_dbfs(),
            band: self.band,
            channel: self.channel,
            channel_offset_hz: self.channel_offset,
            rate_hz: self.rate,
        };
        out.items::<WifiStats>(1).push(Timed {
            time: time_s,
            value: stats,
        });
    }
}

/// The `wifi` graph.
///
/// # Errors
///
/// [`GraphError`] if the graph does not build.
pub fn graph(catalog: &Arc<Catalog>) -> Result<Graph, GraphError> {
    GraphBuilder::new()
        .add("rx", DecoderComponent::new(WifiDecoder::default()))
        .add("table", table::NetworkTable::default())
        .input("rx")
        .connect("rx.mpdus", "table.mpdus")
        .connect("rx.stats", "table.stats")
        .output("table.requests")
        .clock(0.25)
        .status_from("table")
        .catalog(catalog)
        .build()
}

sdrkit_core::choice_enum! {
    /// The choices of the test-signal setting `gen_scene`.
    pub enum GenScene {
        /// The 2.4 GHz scene.
        G2 = ("2.4", "2.4 GHz"),
        /// The 5 GHz scene.
        G5 = ("5", "5 GHz"),
        /// The 6 GHz scene.
        G6 = ("6", "6 GHz"),
    }
}

/// The test-signal setting `gen_scene`: the band of the scene.
pub const GEN_SCENE: Setting<field::Choice<GenScene>> =
    Setting::choice("gen_scene", "Scene", GenScene::G2)
        .section(GEN_SECTION)
        .advanced();
/// The test-signal setting `gen_channel`: the primary channel of a 5 or 6 GHz scene. 0 or a channel the band lacks selects the
/// scene's own channel.
pub const GEN_CHANNEL: Setting<field::I64> = Setting::new("gen_channel", "Primary channel", "", 0)
    .range(0.0, 233.0, 1.0)
    .help("0: the scene's own channel.")
    .section(GEN_SECTION)
    .advanced();

sdrkit_core::settings! {
    /// The settings of the `wifi` stack: the network table's and the test signal's.
    pub enum WifiSetting {
        /// The band.
        Band = table::BAND_SETTING,
        /// The channel.
        Channel = table::CHANNEL_SETTING,
        /// Hop through the hop list.
        Hop = table::HOP,
        /// The dwell while hopping.
        DwellMs = table::DWELL_MS,
        /// Only the preferred scanning channels of 6 GHz.
        PscOnly = table::PSC_ONLY,
        /// The 2.4 GHz channels of the hop list.
        Channels24 = table::CHANNELS_24,
        /// The 5 GHz channels of the hop list.
        Channels5 = table::CHANNELS_5,
        /// The 6 GHz channels of the hop list.
        Channels6 = table::CHANNELS_6,
        /// The network filter text.
        FilterText = table::FILTER_TEXT,
        /// The least level shown.
        MinLevel = table::MIN_LEVEL,
        /// Only open networks.
        OpenOnly = table::OPEN_ONLY,
        /// The band shown.
        BandFilter = table::BAND_FILTER,
        /// The scene of the test signal.
        GenScene = GEN_SCENE,
        /// The primary channel of the test signal.
        GenChannel = GEN_CHANNEL,
    }
}

/// The `wifi` receiver.
///
/// # Errors
///
/// [`GraphError`] if the graph does not build or a value of `cfg` is refused.
pub fn receiver(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    let b = StackGraph::of(crate::typed::shape(ID), graph(&Catalog::of(cfg)?)?)
        .min_rate(MIN_RATE_HZ, "Wi-Fi needs a sample rate of at least 20 Msps");
    route_to(b, "table", &WifiSetting::schema())?
        .command("clear", sdrkit_graph::CommandTarget::every("clear"))
        .command("remove", sdrkit_graph::CommandTarget::of("table", "remove"))
        .with_settings(&cfg.settings)
        .build()
}

/// Adds the Wi-Fi components of this crate to a registry.
pub fn register(r: &mut Registry) {
    r.add("wlan.wifi_rx", || {
        Box::new(DecoderComponent::new(WifiDecoder::default()))
    });
    r.add("wlan.wifi_networks", || {
        Box::new(table::NetworkTable::default())
    });
}

fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    Ok(Box::new(receiver(cfg)?))
}

fn make_generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    let scene = gen_setting(cfg, &GEN_SCENE).unwrap_or(GenScene::G2);
    let ch = gen_setting(cfg, &GEN_CHANNEL)
        .and_then(|c| u32::try_from(c).ok())
        .unwrap_or(0);
    let primary = |band: Band, own: u8| {
        if plan::valid(band, ch) {
            u8::try_from(ch).unwrap_or(own)
        } else {
            own
        }
    };
    let frames = match scene {
        GenScene::G5 => gen::scene_56(Band::G5, primary(Band::G5, 36)),
        GenScene::G6 => gen::scene_56(Band::G6, primary(Band::G6, 5)),
        GenScene::G2 => gen::scene_24(),
    };
    Ok(Box::new(gen::WifiGenerator::new(gen::WifiGenParams {
        rate_hz: cfg.rate_hz,
        frames,
        ..Default::default()
    })))
}

/// The `wifi` stack.
pub static WIFI: StackDescriptor = StackDescriptor {
    id: ID,
    name: "Wi-Fi networks",
    group: "Data and IoT",
    blurb: "The Wi-Fi access points around on 2.4, 5 and 6 GHz from their beacons: names, security, channels and capabilities.",
    tuning: Tuning {
        min_hz: 2400e6,
        max_hz: 7125e6,
        default_hz: 2437e6,
        rate_hz: 20e6,
        min_rate_hz: MIN_RATE_HZ,
        bandwidth_hz: 20e6,
        tune_offset_hz: 0.0,
        carrier_at_centre: false,
        not_circular: false,
        radio_filter_hz: Some(20e6),
        no_offset_tuning: true,
    },
    submodes: &[],
    settings: WifiSetting::schema,
    make_receiver,
    make_generator: Some(make_generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[SignalType::Wifi],
    auto_gain: AutoGain::Radio,
    verify: Some(&crate::verify::WIFI),
    benchmarks: None,
};
