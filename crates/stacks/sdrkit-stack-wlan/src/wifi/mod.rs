//! The Wi-Fi monitor (`wifi`): the frames of access points and client stations on 2.4, 5 and 6 GHz, a network table and a
//! stations table. Decoded: DSSS at 1 and 2 Mbit/s, non-HT OFDM, and HT and VHT PPDUs with one spatial stream at 20 MHz with
//! the convolutional code (A-MPDUs taken apart). HE and EHT PPDUs, and every PPDU with two or more spatial streams or wider
//! than 20 MHz, are counted from their signal fields; one antenna cannot separate two or more streams.
//! Standard: IEEE Std 802.11-2020 clauses 9, 16, 17, 19 and 21; IEEE Std 802.11ax-2021; IEEE Std 802.11be-2024.

mod bench;
mod fields;
pub mod frames;
pub mod gen;
pub mod mac;
pub mod plan;
pub mod shed;
pub mod stations;
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
use fields::Fields;
use shed::{Shed, Shedder};
use sdrkit_graph::{
    no_values, Catalog, Decoder, DecoderComponent, Format, Graph, GraphBuilder, GraphError, InOrder, Item,
    Kind, Outbox, Port, Registry, RunContext, StackGraph, Timed,
};
use sdrkit_proto_wlan_phy::ppdu::{Format as PpduFormat, PpduDecoder, PpduInfo, Read, Undecoded};
use sdrkit_proto_wlan_phy::{dsss, ofdm};
use std::sync::Arc;

/// The stack id.
pub const ID: &str = "wifi";
/// The lowest input rate, Hz. Below [`OFDM_RATE_HZ`] only the 802.11b DSSS frames of 2.4 GHz are received: the part of the
/// 22 MHz main lobe inside the input still despreads. 4 Msps keeps a margin above the rate where it fails (about 2.5 Msps).
pub const MIN_RATE_HZ: f64 = 4e6;
/// The input rate that OFDM (802.11a/g/n/ac/ax) needs: one 20 MHz channel, Hz.
pub const OFDM_RATE_HZ: f64 = 20e6;
/// The rate of the DSSS chain below [`OFDM_RATE_HZ`]: 11 samples a symbol at 1 Msym/s, Hz.
const DSSS_LOW_RATE_HZ: f64 = 11e6;
/// From this rate a 5 or 6 GHz capture is split into its 20 MHz channels.
pub const WIDE_RATE_HZ: f64 = 43.5e6;
/// The rate both receivers run at from [`OFDM_RATE_HZ`] on, Hz.
pub const CHAIN_RATE_HZ: f64 = 20e6;
/// The work an OFDM chain may be behind its input before it skips the HT and VHT data fields that no helper has room for, s.
const BEHIND_S: f64 = 0.05;
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
        /// HT (clause 19).
        Ht = ("ht", "HT"),
        /// VHT (clause 21).
        Vht = ("vht", "VHT"),
    }
}

impl PhyKind {
    /// "DSSS" or "OFDM".
    pub fn name(self) -> &'static str {
        match self {
            PhyKind::Dsss => "DSSS",
            PhyKind::Ofdm => "OFDM",
            PhyKind::Ht => "HT",
            PhyKind::Vht => "VHT",
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
    /// The PPDU format of an OFDM frame: `None` is non-HT, else HT or VHT.
    pub format: Option<PpduFormat>,
    /// The MCS of an HT or VHT PPDU.
    pub mcs: Option<u8>,
    /// The MPDU came in an A-MPDU.
    pub aggregated: bool,
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

/// The reasons a PPDU's data is not decoded, in the order of [`PpduCounters::not_decoded`].
pub const UNDECODED: [Undecoded; 14] = [
    Undecoded::Streams,
    Undecoded::Width,
    Undecoded::Ldpc,
    Undecoded::Stbc,
    Undecoded::MultiUser,
    Undecoded::TriggerBased,
    Undecoded::HeData,
    Undecoded::EhtData,
    Undecoded::SignalField,
    Undecoded::Mcs,
    Undecoded::Length,
    Undecoded::Data,
    Undecoded::Other,
    Undecoded::Busy,
];

/// The PPDU formats, in the order of [`PpduCounters::seen`].
pub const FORMATS: [PpduFormat; 8] = [
    PpduFormat::Ht,
    PpduFormat::Vht,
    PpduFormat::HeSu,
    PpduFormat::HeErSu,
    PpduFormat::HeMu,
    PpduFormat::HeTb,
    PpduFormat::He,
    PpduFormat::Eht,
];

/// The counters of the HT, VHT, HE and EHT PPDUs, summed over every chain the stack has run.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PpduCounters {
    /// PPDUs whose signal field was read (for HE and EHT: whose RL-SIG was found), by format in the order of [`FORMATS`].
    pub seen: [u64; 8],
    /// PPDUs whose data was decoded.
    pub decoded: u64,
    /// PPDUs whose data was not decoded, by reason in the order of [`UNDECODED`].
    pub not_decoded: [u64; 14],
    /// MPDUs of these PPDUs with a good FCS.
    pub mpdus: u64,
    /// MPDUs of these PPDUs with a bad FCS.
    pub bad_fcs: u64,
    /// A-MPDU delimiter positions with a bad CRC or signature.
    pub bad_delimiters: u64,
    /// The last PPDU that was not decoded.
    pub last: Option<(PpduInfo, Undecoded)>,
}

impl PpduCounters {
    /// The counts since `base`.
    pub fn since(mut self, base: &PpduCounters) -> PpduCounters {
        for (a, b) in self.seen.iter_mut().zip(base.seen) {
            *a = a.saturating_sub(b);
        }
        for (a, b) in self.not_decoded.iter_mut().zip(base.not_decoded) {
            *a = a.saturating_sub(b);
        }
        self.decoded = self.decoded.saturating_sub(base.decoded);
        self.mpdus = self.mpdus.saturating_sub(base.mpdus);
        self.bad_fcs = self.bad_fcs.saturating_sub(base.bad_fcs);
        self.bad_delimiters = self.bad_delimiters.saturating_sub(base.bad_delimiters);
        self
    }

    /// PPDUs counted for format `f`.
    pub fn seen_of(&self, f: PpduFormat) -> u64 {
        FORMATS.iter().position(|&x| x == f).map_or(0, |i| self.seen[i])
    }

    /// PPDUs not decoded for reason `why`.
    pub fn not_decoded_for(&self, why: Undecoded) -> u64 {
        UNDECODED.iter().position(|&x| x == why).map_or(0, |i| self.not_decoded[i])
    }

    fn note(&mut self, info: &PpduInfo, why: Option<Undecoded>) {
        if let Some(i) = FORMATS.iter().position(|&x| x == info.format) {
            self.seen[i] += 1;
        }
        match why {
            None => self.decoded += 1,
            Some(w) => {
                if let Some(i) = UNDECODED.iter().position(|&x| x == w) {
                    self.not_decoded[i] += 1;
                }
                self.last = Some((*info, w));
            }
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
    /// HT, VHT, HE and EHT counters.
    pub ppdu: PpduCounters,
    /// The channels listened to at once.
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
    /// What the decoder leaves out because live input came faster than it works.
    pub shed: Shed,
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
    Ofdm(Box<ofdm::OfdmReceiver>, Box<PpduDecoder>),
}

/// One 20 MHz channel: moved to 0 Hz and resampled to the rate of its receiver (20 Msps, or 11 Msps for DSSS from a lower
/// input rate).
struct Chain {
    ch: Channelizer,
    /// The output rate of the channelizer, Hz.
    out_rate: f64,
    rx: Rx,
    buf: Vec<Complex32>,
    /// How far the OFDM receiver is behind its input, s of work beyond the signal time (it falls back to 0 on quiet input).
    behind_s: f64,
    /// Signal time of the chain's first sample, s.
    t0: f64,
    /// A DSSS chain on a channel next to the tuned one: its counters are left out and its frames are kept only when no
    /// chain on their own channel has a better copy.
    neighbour: bool,
    /// The frames of a DSSS chain from the last piece, for a run on a helper thread.
    found: Vec<Timed<WifiMpdu>>,
    band: Option<Band>,
    channel: u32,
}

impl Chain {
    /// Runs a piece of input through a DSSS chain; its frames are left in `found`.
    fn run_dsss(&mut self, x: &[Complex32]) {
        self.buf.clear();
        self.ch.process(x, &mut self.buf);
        let (t0, band, channel, out_rate) = (self.t0, self.band, self.channel, self.out_rate);
        let found = &mut self.found;
        if let Rx::Dsss(r) = &mut self.rx {
            r.process(&self.buf, |f| {
                let rate = f.header.rate.units_100k() as f64 / 10.0;
                let mut m = mpdu(
                    f.mpdu,
                    PhyKind::Dsss,
                    rate,
                    f.level_dbfs,
                    f.offset_hz,
                    f.snr_db,
                    t0 + f.sample as f64 / out_rate,
                    channel,
                    band,
                );
                m.value.short_preamble = f.preamble == dsss::Preamble::Short;
                found.push(m);
            });
        }
    }

    /// A chain of `width` Hz at `out_rate` Hz around `offset` Hz of an input at `rate` Hz.
    #[allow(clippy::too_many_arguments)]
    fn new(
        rate: f64,
        offset: f64,
        width: f64,
        out_rate: f64,
        rx: Rx,
        t0: f64,
        band: Option<Band>,
        channel: u32,
    ) -> Option<Chain> {
        let mut cfg = ChannelConfig::new(rate, offset, width, out_rate);
        cfg.exact_rate = true;
        Some(Chain {
            ch: Channelizer::new(cfg).ok()?,
            out_rate,
            rx,
            buf: Vec::new(),
            behind_s: 0.0,
            t0,
            neighbour: false,
            found: Vec::new(),
            band,
            channel,
        })
    }

    /// Runs a piece of input. `share` is the part of the processor time this chain may use: 1 over the chains running. DSSS
    /// frames go to `out`; the OFDM frames and data fields go to `fields`, which lets them out in order. With `shed_data`
    /// and live input the HT and VHT data fields are counted as busy and not decoded; a chain behind its input skips those
    /// that no helper has room for.
    fn run(
        &mut self,
        x: &[Complex32],
        share: f64,
        out: &mut Vec<Timed<WifiMpdu>>,
        fields: &mut Fields,
        shed_data: bool,
    ) {
        if matches!(self.rx, Rx::Dsss(_)) {
            self.run_dsss(x);
            out.append(&mut self.found);
            return;
        }
        self.buf.clear();
        self.ch.process(x, &mut self.buf);
        let (t0, band, channel) = (self.t0, self.band, self.channel);
        let out_rate = self.out_rate;
        let at = |n: u64| t0 + n as f64 / out_rate;
        match &mut self.rx {
            Rx::Dsss(_) => {}
            Rx::Ofdm(r, dec) => {
                // With live input, a chain more than BEHIND_S behind the signal skips the data fields that no helper has
                // room for, and a decoder that sheds data skips them all. The time spent waiting for the window counts.
                let behind = self.behind_s >= BEHIND_S;
                let started = fields.work_s();
                let signal_s = self.buf.len() as f64 / out_rate * share;
                let fields = std::cell::RefCell::new(fields);
                r.process_with(
                    &self.buf,
                    |f| {
                        let rate = f.signal.rate.mbps() as f64;
                        fields.borrow_mut().frame(mpdu(
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
                    },
                    |v| match dec.read(v) {
                        Read::NonHt => false,
                        Read::Seen { info, why } => {
                            fields.borrow_mut().note(info, why);
                            true
                        }
                        Read::Data(plan) => {
                            fields.borrow_mut().field(v, plan, at(v.sample), channel, band, shed_data, behind);
                            true
                        }
                    },
                );
                let work_s = fields.borrow().work_s() - started;
                self.behind_s = (self.behind_s + work_s - signal_s).max(0.0);
            }
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
            format: None,
            mcs: None,
            aggregated: false,
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
    /// DSSS frames that only a chain on a neighbouring channel decoded.
    neighbour_frames: u64,
    /// The DSSS chains of a piece on the helper threads, when the run context has helpers.
    dsss_jobs: Option<InOrder<ChainJob, ()>>,
    /// The chains while their DSSS part is on the helper threads.
    back: Vec<Chain>,
    /// The piece of input for the helper threads, reused while no helper holds it.
    samples: Arc<Vec<Complex32>>,
    fields: Fields,
    band: Option<Band>,
    channel: u32,
    channel_offset: f64,
    nearest: bool,
    input: u64,
    level: IntervalLevel,
    out: Vec<Timed<WifiMpdu>>,
    /// The input format of the chains, for a rebuild when the shed level changes.
    format: Option<Format>,
    shedder: Shedder,
    /// The shed level that the chains are built for.
    shed: Shed,
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

    fn counters(&self) -> (DsssCounters, OfdmCounters, PpduCounters) {
        let (mut d, mut o) = (self.kept_dsss, self.kept_ofdm);
        for c in &self.chains {
            match &c.rx {
                Rx::Dsss(_) if c.neighbour => {}
                Rx::Dsss(r) => d.add(r.stats()),
                Rx::Ofdm(r, _) => o.add(r.stats()),
            }
        }
        d.frames += self.neighbour_frames;
        (d, o, self.fields.ppdu)
    }

    /// The channels that the chains listen to at once, counted without allocating. Chains on a channel that is not known
    /// listen at the tuned centre only.
    fn channels_at_once(&self) -> u32 {
        let mut seen = 0u32;
        for (i, c) in self.chains.iter().enumerate() {
            seen += u32::from(self.chains[..i].iter().all(|d| d.channel != c.channel));
        }
        seen
    }

    /// Runs the chains on a piece and lets out what is done: everything when the input is not live.
    fn run_chains(&mut self, x: &[Complex32]) {
        self.out.clear();
        let share = 1.0 / self.chains.len().max(1) as f64;
        let shed_data = self.shed == Shed::Data;
        let dsss = self.chains.iter().filter(|c| matches!(c.rx, Rx::Dsss(_))).count();
        match self.dsss_jobs.as_mut() {
            // Several DSSS chains run on the helper threads and come back in the order of the chains; the OFDM chains follow
            // in line.
            Some(jobs) if dsss > 1 => {
                let samples = match Arc::get_mut(&mut self.samples) {
                    Some(v) => {
                        v.clear();
                        v.extend_from_slice(x);
                        Arc::clone(&self.samples)
                    }
                    None => {
                        self.samples = Arc::new(x.to_vec());
                        Arc::clone(&self.samples)
                    }
                };
                // The DSSS chains come back into `back` in order; the OFDM chains follow them there. Both vectors keep their
                // size, so this allocates nothing.
                let (out, back) = (&mut self.out, &mut self.back);
                back.clear();
                let kept = &mut *back;
                for c in self.chains.drain(..dsss) {
                    let mut c = Some(c);
                    if !jobs.give(|j| (j.chain, j.samples) = (c.take(), Some(Arc::clone(&samples)))) {
                        // The window holds every DSSS chain of a capture: this does not happen.
                        if let Some(mut c) = c {
                            c.run_dsss(&samples);
                            out.append(&mut c.found);
                            kept.push(c);
                        }
                    }
                }
                jobs.take_all(|j, _| {
                    j.samples = None;
                    if let Some(mut c) = j.chain.take() {
                        out.append(&mut c.found);
                        kept.push(c);
                    }
                });
                drop(samples);
                self.back.append(&mut self.chains);
                std::mem::swap(&mut self.chains, &mut self.back);
                for c in &mut self.chains[dsss..] {
                    c.run(x, share, &mut self.out, &mut self.fields, shed_data);
                }
            }
            _ => {
                for c in &mut self.chains {
                    c.run(x, share, &mut self.out, &mut self.fields, shed_data);
                }
            }
        }
        self.fields.flush(!self.fields.live);
        self.out.append(&mut self.fields.ready);
        self.out.sort_by(|a, b| a.time.total_cmp(&b.time));
        if self.chains.iter().any(|c| c.neighbour) {
            self.neighbour_frames += keep_best_copies(&mut self.out, self.channel);
        }
    }

    /// The counters and the input level as a stats item at the end of the input so far.
    fn stats(&mut self) -> Timed<WifiStats> {
        let (dsss, ofdm, ppdu) = self.counters();
        let time_s = self.input as f64 / self.rate.max(1.0);
        let stats = WifiStats {
            time_s,
            dsss,
            ofdm,
            ppdu,
            sub_channels: self.channels_at_once(),
            level_dbfs: self.level.take_dbfs(),
            band: self.band,
            channel: self.channel,
            channel_offset_hz: self.channel_offset,
            rate_hz: self.rate,
            shed: self.shed,
        };
        Timed {
            time: time_s,
            value: stats,
        }
    }

    /// Sets the shed level: the chains are built again when the level adds or removes the neighbour channels.
    fn set_shed(&mut self, level: Shed) {
        let rebuild = (level == Shed::Nothing) != (self.shed == Shed::Nothing);
        self.shed = level;
        if let (true, Some(f)) = (rebuild, self.format.clone()) {
            self.rebuild(&f);
        }
    }

    /// Counts a piece of live input that held `signal_s` of signal and took `work_s`, and sheds or takes back work.
    fn worked(&mut self, work_s: f64, signal_s: f64) {
        let level = self.shedder.step(work_s, signal_s);
        if level != self.shed {
            self.set_shed(level);
        }
    }

    fn rebuild(&mut self, f: &Format) {
        self.format = Some(f.clone());
        let neighbours = self.shed == Shed::Nothing;
        let (d, o, _) = self.counters();
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
        let ofdm = rate >= OFDM_RATE_HZ * 0.999_999;
        if matches!(band, None | Some(Band::G2)) {
            // Below the OFDM rate the DSSS chain keeps what the input holds of the 22 MHz main lobe: the Barker correlation
            // still finds the symbols, with less margin.
            let (out, width) = if ofdm {
                (CHAIN_RATE_HZ, CHAIN_WIDTH_HZ)
            } else {
                (DSSS_LOW_RATE_HZ, (0.85 * rate).min(0.89 * DSSS_LOW_RATE_HZ))
            };
            let mut dsss = vec![(offset, width, out, ch)];
            // Every other 2.4 GHz channel whose main lobe reaches far enough into the capture: the part of the lobe inside
            // still despreads, as below the OFDM rate.
            if neighbours && band == Some(Band::G2) && centre > 0.0 {
                for (off, c) in self::neighbours(centre, rate, ch) {
                    let half = 0.5 * rate - off.abs();
                    if half >= 0.5 * MIN_RATE_HZ {
                        dsss.push((off, (1.7 * half).min(0.89 * DSSS_LOW_RATE_HZ), DSSS_LOW_RATE_HZ, c));
                    }
                }
            }
            for (k, (off, width, out, c)) in dsss.into_iter().enumerate() {
                if let Ok(r) = dsss::DsssReceiver::new((out / 1e6) as usize) {
                    let chain = Chain::new(rate, off, width, out, Rx::Dsss(Box::new(r)), t0, band, c);
                    self.chains.extend(chain.map(|mut c| {
                        c.neighbour = k > 0;
                        c
                    }));
                }
            }
        }
        if !ofdm {
            return;
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
        // The channel nearest the signal offset comes first: it is the one that stays when the neighbours are shed.
        let nearest = offsets
            .iter()
            .enumerate()
            .min_by(|a, b| (a.1 .0 - offset).abs().total_cmp(&(b.1 .0 - offset).abs()))
            .map_or(0, |(i, _)| i);
        offsets.swap(0, nearest);
        // On 2.4 GHz, also every other channel whose 16.6 MHz of OFDM tones lie wholly inside the capture.
        if band == Some(Band::G2) && centre > 0.0 {
            for (off, c) in self::neighbours(centre, rate, ch) {
                if plan::inside((centre + off) / 1e6, centre / 1e6, rate) {
                    offsets.push((off, c));
                }
            }
        }
        if !neighbours {
            offsets.truncate(1);
        }
        for (k, (off, c)) in offsets.into_iter().enumerate() {
            let r = Rx::Ofdm(Box::new(ofdm::OfdmReceiver::new()), Box::new(PpduDecoder::new()));
            let chain = Chain::new(rate, off, CHAIN_WIDTH_HZ, CHAIN_RATE_HZ, r, t0, band, c);
            self.chains.extend(chain.map(|mut c| {
                c.neighbour = k > 0;
                c
            }));
        }
    }
}

/// The time within which two chains give their copies of one frame: more than the difference of their filter delays, s.
const COPY_WINDOW_S: f64 = 400e-6;

/// The most DSSS chains of one capture: one for each 2.4 GHz channel.
const MAX_DSSS_CHAINS: usize = 14;

/// A DSSS chain with a piece of input, for a helper thread.
#[derive(Default)]
struct ChainJob {
    chain: Option<Chain>,
    samples: Option<Arc<Vec<Complex32>>>,
}

/// Keeps one copy of each DSSS frame that chains on different channels both decoded (the same bytes within
/// [`COPY_WINDOW_S`]): the copy with the best SNR, which the chain on the frame's own channel gives. A DSSS frame despreads
/// 5 MHz off its centre too. Returns the frames kept that no chain on channel `tuned` decoded.
fn keep_best_copies(out: &mut Vec<Timed<WifiMpdu>>, tuned: u32) -> u64 {
    let mut only_neighbours = 0;
    let mut i = 0;
    while i < out.len() {
        if out[i].value.phy == PhyKind::Dsss {
            let mut on_tuned = out[i].value.channel == tuned;
            let mut j = i + 1;
            while j < out.len() && out[j].time - out[i].time < COPY_WINDOW_S {
                let (a, b) = (&out[i].value, &out[j].value);
                if b.phy == PhyKind::Dsss && b.channel != a.channel && b.bytes == a.bytes {
                    let other = out.remove(j);
                    on_tuned |= other.value.channel == tuned;
                    if other.value.snr_db > out[i].value.snr_db {
                        out[i] = other;
                    }
                } else {
                    j += 1;
                }
            }
            only_neighbours += u64::from(!on_tuned);
        }
        i += 1;
    }
    only_neighbours
}

/// The 2.4 GHz channels other than `tuned` whose centres lie inside a capture of `rate` Hz around `centre` Hz, with their
/// offsets from `centre`, Hz.
fn neighbours(centre: f64, rate: f64, tuned: u32) -> Vec<(f64, u32)> {
    plan::all_channels(Band::G2)
        .into_iter()
        .filter(|&c| c != tuned)
        .filter_map(|c| centre_mhz(Band::G2, c).map(|f| (f * 1e6 - centre, c)))
        .filter(|(off, _)| off.abs() < 0.5 * rate)
        .collect()
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
    fn attach(&mut self, ctx: &RunContext) {
        // Data fields still in the window belong to the old context: they leave first.
        self.fields.flush(true);
        let ready = std::mem::take(&mut self.fields.ready);
        let ppdu = self.fields.ppdu;
        self.fields = Fields::new(&ctx.pool, ctx.pool.workers(), ctx.live);
        self.dsss_jobs = (ctx.pool.workers() > 0).then(|| {
            InOrder::new(&ctx.pool, ctx.pool.workers(), MAX_DSSS_CHAINS, || (), ChainJob::default, |_, j: &mut ChainJob| {
                if let (Some(c), Some(x)) = (&mut j.chain, &j.samples) {
                    c.run_dsss(x);
                }
            })
        });
        self.fields.ready = ready;
        self.fields.ppdu = ppdu;
        // What a live run shed is taken back: the new input may be a recording.
        self.shedder = Shedder::default();
        if self.shed != Shed::Nothing {
            self.set_shed(Shed::Nothing);
        }
    }
    fn process(&mut self, x: &[Complex32], first: u64, out: &mut Outbox) {
        self.input = first + x.len() as u64;
        self.level.push(x);
        // Only live input sheds work: a recording waits for the decoder.
        let started = self.fields.live.then(std::time::Instant::now);
        self.run_chains(x);
        if let Some(t) = started {
            self.worked(t.elapsed().as_secs_f64(), x.len() as f64 / self.rate.max(1.0));
        }
        out.items::<WifiMpdu>(0).append(&mut self.out);
        let stats = self.stats();
        out.items::<WifiStats>(1).push(stats);
    }
    fn finish(&mut self, out: &mut Outbox) {
        if self.fields.flush_all() {
            self.out.clear();
            self.out.append(&mut self.fields.ready);
            self.out.sort_by(|a, b| a.time.total_cmp(&b.time));
            out.items::<WifiMpdu>(0).append(&mut self.out);
            let stats = self.stats();
            out.items::<WifiStats>(1).push(stats);
        }
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
        .min_rate(
            MIN_RATE_HZ,
            "Wi-Fi works from 4 Msps with the 802.11b (DSSS) frames of 2.4 GHz only; OFDM and the 5 and 6 GHz bands need 20 Msps",
        );
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
    blurb: "The Wi-Fi access points around on 2.4, 5 and 6 GHz from their beacons: names, security, channels and capabilities. Works from 4 Msps with the 802.11b (DSSS) frames of 2.4 GHz only; OFDM and the 5 and 6 GHz bands need 20 Msps.",
    tuning: Tuning {
        kind: Some(sdrkit_core::TuningKind::Channels),
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
        ..Tuning::DEFAULT
    },
    submodes: &[],
    settings: WifiSetting::schema,
    make_receiver,
    make_generator: Some(make_generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[SignalType::Wifi],
    auto_gain: AutoGain::Radio,
    verify: Some(&crate::verify::WIFI),
    benchmarks: Some(bench::benchmarks),
};

#[cfg(test)]
mod tests {
    use super::*;
    use sdrkit_proto_wlan_phy::ht::{self, HtSig, Mcs, PilotPattern};
    use sdrkit_proto_wlan_phy::mac as phymac;
    use sdrkit_proto_wlan_phy::vht::{self, VhtMcs};

    fn mpdu(n: usize, tag: u8) -> Vec<u8> {
        let mut f = vec![0x08, 0x01, 0, 0, 2, 0, 0, 0, 0, 1, 6, 1, 2, 3, 4, tag, 2, 0, 0, 0, 0, 1, 0x10, 0];
        f.extend((0..n).map(|i| tag.wrapping_add(i as u8)));
        phymac::push_fcs(&mut f);
        f
    }

    /// An OFDM chain at 20 Msps with no offset.
    fn chain() -> Chain {
        let rx = Rx::Ofdm(Box::new(ofdm::OfdmReceiver::new()), Box::new(PpduDecoder::new()));
        Chain::new(20e6, 0.0, CHAIN_WIDTH_HZ, CHAIN_RATE_HZ, rx, 0.0, Some(Band::G5), 36).unwrap()
    }

    /// Runs `x` through chain `c` in pieces of `piece` samples and lets out everything after each piece.
    fn run_all(c: &mut Chain, fields: &mut Fields, x: &[Complex32], piece: usize) -> Vec<Timed<WifiMpdu>> {
        let mut out = Vec::new();
        for p in x.chunks(piece) {
            c.run(p, 1.0, &mut out, fields, false);
            fields.flush(!fields.live);
            out.append(&mut fields.ready);
        }
        fields.flush(true);
        out.append(&mut fields.ready);
        out
    }

    fn padded(parts: &[Vec<Complex32>]) -> Vec<Complex32> {
        let mut x = vec![Complex32::default(); 2000];
        for p in parts {
            x.extend_from_slice(p);
            x.extend(std::iter::repeat_n(Complex32::default(), 1500));
        }
        x.extend(std::iter::repeat_n(Complex32::default(), 4000));
        x
    }

    #[test]
    fn wifi_chain_decodes_ht_and_vht_ampdus_and_counts_what_it_cannot() {
        let parts = [mpdu(200, 1), mpdu(700, 2)];
        let mut a = Vec::new();
        for (i, m) in parts.iter().enumerate() {
            phymac::push_ampdu_subframe(&mut a, m, i == 1);
        }
        let sig = |mcs: u8, aggregation: bool| HtSig {
            mcs,
            smoothing: true,
            not_sounding: true,
            aggregation,
            ..HtSig::default()
        };
        let (mut h, mut v, mut two, mut one) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
        ht::build_ppdu_sig(&a, sig(4, true), Mcs(4), 0x21, PilotPattern::Rotating, &mut h);
        vht::build_ppdu(&a, VhtMcs(8), true, 0x44, &mut v);
        ht::build_ppdu_sig(&parts[0], sig(12, false), Mcs(4), 0x21, PilotPattern::Fixed, &mut two);
        ht::build_ppdu_sig(&parts[0], sig(0, false), Mcs(0), 0x21, PilotPattern::Fixed, &mut one);
        let x = padded(&[h, v, two, one]);
        let mut c = chain();
        let mut fields = Fields::default();
        let out = run_all(&mut c, &mut fields, &x, 5000);
        type Got<'a> = (&'a [u8], Option<PpduFormat>, Option<u8>, bool);
        let got: Vec<Got<'_>> = out
            .iter()
            .map(|m| (&m.value.bytes[..], m.value.format, m.value.mcs, m.value.aggregated))
            .collect();
        let body = |m: &Vec<u8>| m[..m.len() - 4].to_vec();
        let (b0, b1) = (body(&parts[0]), body(&parts[1]));
        assert_eq!(
            got,
            [
                (&b0[..], Some(PpduFormat::Ht), Some(4), true),
                (&b1[..], Some(PpduFormat::Ht), Some(4), true),
                (&b0[..], Some(PpduFormat::Vht), Some(8), true),
                (&b1[..], Some(PpduFormat::Vht), Some(8), true),
                (&b0[..], Some(PpduFormat::Ht), Some(0), false),
            ]
        );
        assert!((out[0].value.rate_mbps - 39.0).abs() < 1e-9 && (out[2].value.rate_mbps - 86.7).abs() < 0.04);
        let p = fields.ppdu;
        assert_eq!((p.seen_of(PpduFormat::Ht), p.seen_of(PpduFormat::Vht)), (3, 1));
        assert_eq!((p.decoded, p.mpdus, p.bad_fcs), (3, 5, 0));
        assert_eq!(p.not_decoded_for(Undecoded::Streams), 1);
        let (info, why) = p.last.unwrap();
        assert_eq!((info.streams, info.mcs, why.text()), (2, Some(12), "two or more spatial streams"));
        // The non-HT receiver counted none of them as a frame of its own.
        let Rx::Ofdm(r, _) = &c.rx else { unreachable!() };
        assert_eq!((r.stats().frames, r.stats().bad_fcs), (0, 0));
    }

    /// The MPDUs that a decoder on channel 6 at `rate` Hz takes from beacons sent at `positions` (Hz from the centre) as
    /// DSSS and as 6 Mbit/s OFDM, with the channel each was heard on, and the channels it listens to at once.
    fn heard_around_channel_6(rate: f64, positions: &[(f64, gen::Phy)]) -> (Vec<(u8, PhyKind, u32)>, u32) {
        use sdrkit_core::Generator;
        let frames = positions
            .iter()
            .enumerate()
            .map(|(i, &(at, phy))| {
                let mut f = gen::GenFrame::new(mpdu(60, i as u8 + 1), phy).gap(1e-3);
                f.position_hz = at;
                f
            })
            .collect();
        let mut g = gen::WifiGenerator::new(gen::WifiGenParams {
            rate_hz: rate,
            frames,
            looped: false,
            ..Default::default()
        });
        let mut x = vec![Complex32::default(); (0.03 * rate) as usize];
        g.generate(&mut x);
        let mut dec = WifiDecoder::default();
        dec.configure(&Format::iq(rate, 2437e6, 2437e6)).unwrap();
        let mut got = Vec::new();
        for p in x.chunks(65_536) {
            dec.input += p.len() as u64;
            dec.run_chains(p);
            got.extend(dec.out.drain(..).map(|m| (m.value.bytes[15], m.value.phy, m.value.channel)));
        }
        got.sort_unstable_by_key(|g| g.0);
        got.dedup();
        (got, dec.channels_at_once())
    }

    #[test]
    fn beacons_on_every_channel_inside_the_capture_are_heard_on_their_own_channel() {
        use gen::Phy::{Dsss, Ofdm};
        // At 20 Msps on channel 6: DSSS on channels 5 and 7 from the part of their main lobe inside; OFDM only on channel 6.
        let (got, at_once) = heard_around_channel_6(20e6, &[(-5e6, Dsss), (0.0, Dsss), (5e6, Dsss), (0.0, Ofdm)]);
        assert_eq!(
            got,
            [(1, PhyKind::Dsss, 5), (2, PhyKind::Dsss, 6), (3, PhyKind::Dsss, 7), (4, PhyKind::Ofdm, 6)]
        );
        assert_eq!(at_once, 3);
        // At 40 Msps: DSSS also on channel 9, and OFDM on channels 5 and 7, whose tones lie wholly inside.
        let (got, _) =
            heard_around_channel_6(40e6, &[(15e6, Dsss), (-5e6, Ofdm), (5e6, Ofdm), (15e6, Ofdm)]);
        assert_eq!(got, [(1, PhyKind::Dsss, 9), (2, PhyKind::Ofdm, 5), (3, PhyKind::Ofdm, 7)]);
    }

    /// A decoder on channel 6 at 20 Msps with live input, and 30 ms of beacons: DSSS on channels 5, 6 and 7, non-HT OFDM at 6 and
    /// at 54 Mbit/s and one HT PPDU on channel 6. The tag of each frame is its place in this list, from 1.
    fn live_on_channel_6() -> (WifiDecoder, Vec<Complex32>) {
        use gen::Phy::{Dsss, Ofdm};
        use sdrkit_core::Generator;
        let mut frames: Vec<gen::GenFrame> = [(-5e6, Dsss), (0.0, Dsss), (5e6, Dsss), (0.0, Ofdm), (0.0, Ofdm)]
            .iter()
            .enumerate()
            .map(|(i, &(at, phy))| {
                let mut f = gen::GenFrame::new(mpdu(60, i as u8 + 1), phy).gap(1e-3);
                f.position_hz = at;
                f
            })
            .collect();
        frames[4] = frames[4].clone().ofdm_rate(ofdm::Rate::R54);
        let mut g = gen::WifiGenerator::new(gen::WifiGenParams {
            rate_hz: 20e6,
            frames,
            looped: false,
            ..Default::default()
        });
        let mut x = vec![Complex32::default(); 600_000];
        g.generate(&mut x);
        let mut ht = Vec::new();
        ht::build_ppdu(&mpdu(100, 6), Mcs(2), false, 0x21, PilotPattern::Fixed, &mut ht);
        x.extend(padded(&[ht]));
        let mut dec = WifiDecoder::default();
        dec.fields.live = true;
        dec.configure(&Format::iq(20e6, 2437e6, 2437e6)).unwrap();
        (dec, x)
    }

    /// The tags and channels of the frames that `dec` takes from `x`.
    fn tags(dec: &mut WifiDecoder, x: &[Complex32]) -> Vec<(u8, u32)> {
        let mut got = Vec::new();
        for p in x.chunks(65_536) {
            dec.input += p.len() as u64;
            dec.run_chains(p);
            got.extend(dec.out.drain(..).map(|m| (m.value.bytes[15], m.value.channel)));
        }
        dec.fields.flush(true);
        got.extend(dec.fields.ready.drain(..).map(|m| (m.value.bytes[15], m.value.channel)));
        got.sort_unstable();
        got.dedup();
        got
    }

    #[test]
    fn a_decoder_behind_live_input_sheds_neighbours_then_data_and_takes_them_back() {
        let (mut dec, x) = live_on_channel_6();
        let all = [(1, 5), (2, 6), (3, 7), (4, 6), (5, 6), (6, 6)];
        assert_eq!(tags(&mut dec, &x), all);
        assert_eq!((dec.shed, dec.channels_at_once()), (Shed::Nothing, 3));
        // Work that takes twice the signal time: the chains next to the tuned channel go first. The chain of the tuned
        // channel still despreads the DSSS frames 5 MHz off, and gives them its own channel.
        dec.worked(0.4, 0.2);
        assert_eq!((dec.shed, dec.channels_at_once()), (Shed::Neighbours, 1));
        assert_eq!(tags(&mut dec, &x), [(1, 6), (2, 6), (3, 6), (4, 6), (5, 6), (6, 6)]);
        // Still behind: the HT data field is left out and counted; the DSSS and non-HT frames of the tuned channel, the
        // beacons among them, stay.
        dec.worked(0.4, 0.2);
        assert_eq!(dec.shed, Shed::Data);
        assert_eq!(tags(&mut dec, &x), [(1, 6), (2, 6), (3, 6), (4, 6), (5, 6)]);
        assert_eq!(dec.fields.ppdu.not_decoded_for(Undecoded::Busy), 1);
        assert_eq!(dec.stats().value.shed, Shed::Data);
        // Light work for the hold time, twice: everything is decoded again.
        for _ in 0..2 {
            for _ in 0..(shed::HOLD_S + 2.0) as usize * 10 {
                dec.worked(0.01, 0.1);
            }
        }
        assert_eq!(dec.shed, Shed::Nothing);
        assert_eq!(tags(&mut dec, &x), all);
    }

    #[test]
    fn a_recording_sheds_nothing_however_slow_the_work() {
        use sdrkit_graph::Outbox;
        let (mut dec, x) = live_on_channel_6();
        dec.fields.live = false;
        let mut out = Outbox::new(&[Kind::typed::<WifiMpdu>(), Kind::typed::<WifiStats>()]);
        let mut first = 0;
        for p in x.chunks(65_536) {
            dec.process(p, first, &mut out);
            first += p.len() as u64;
        }
        assert_eq!((dec.shed, dec.shedder), (Shed::Nothing, Shedder::default()));
        assert_eq!(dec.channels_at_once(), 3);
        // A decoder that shed while live takes everything back when its input becomes a recording.
        dec.fields.live = true;
        dec.worked(0.4, 0.2);
        assert_eq!((dec.shed, dec.channels_at_once()), (Shed::Neighbours, 1));
        dec.attach(&RunContext::default());
        assert_eq!((dec.shed, dec.shedder, dec.channels_at_once()), (Shed::Nothing, Shedder::default(), 3));
    }

    #[test]
    fn chains_on_helper_threads_give_what_they_give_in_line() {
        use sdrkit_core::Generator;
        let mut g = gen::WifiGenerator::new(gen::WifiGenParams::default());
        let mut x = vec![Complex32::default(); 4_000_000];
        g.generate(&mut x);
        let run = |helpers: usize| {
            let mut dec = WifiDecoder::default();
            if helpers > 0 {
                let pool = sdrkit_graph::Pool::new(helpers);
                dec.attach(&RunContext {
                    pool,
                    ..RunContext::default()
                });
            }
            dec.configure(&Format::iq(20e6, 2437e6, 2437e6)).unwrap();
            let mut got = Vec::new();
            for p in x.chunks(65_536) {
                dec.input += p.len() as u64;
                dec.run_chains(p);
                got.append(&mut dec.out);
            }
            (got, dec.counters())
        };
        let (one, counters) = run(0);
        assert!(one.len() > 20);
        assert!(run(3) == (one, counters));
    }

    #[test]
    fn wifi_chain_behind_its_input_counts_data_fields_as_busy() {
        let mut one = Vec::new();
        ht::build_ppdu(&mpdu(100, 3), Mcs(2), false, 0x21, PilotPattern::Fixed, &mut one);
        let x = padded(&[one]);
        let mut c = chain();
        c.behind_s = 1.0;
        let mut fields = Fields::default();
        fields.live = true;
        let out = run_all(&mut c, &mut fields, &x, x.len());
        assert!(out.is_empty());
        assert_eq!(fields.ppdu.not_decoded_for(Undecoded::Busy), 1);
        assert!(c.behind_s < 1.0);
    }
}
