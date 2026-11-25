//! The DRM receiver: its components and graph.
//!
//! Standard: ETSI ES 201 980.

use super::audio::{parse_superframe, TextReceiver};
use super::data::{MotObject, PacketService, PacketStats};
use super::disperse;
use super::fac::{
    language_name, programme_type_name, sdc_data_len, service_counts, AppInfo, AudioInfo, Entity,
    Fac, Sdc,
};
use super::interleave::CellDeinterleaver;
use super::mlc::{higher_part_cells, Mlc};
use super::params::{
    fac_rate, level_rates, msc_qam, occupancy_khz, overall_rate_pct, sdc_qam, sdc_rates,
    search_layout, CellMap, Robustness,
};
use super::results as res;
use super::tx::audio_frames;
use super::ID;
use crate::component::ClearCommand;
use sdrkit_algo_dsp::fft::Fft;
use sdrkit_algo_dsp::resample::Resampler;
use sdrkit_algo_modem::ofdm::{CellFrame, FrameDemodulator};
use sdrkit_core::interface::{Fields, Hertz, Seconds};
use sdrkit_core::media::{AudioProps, Codec, MediaFormat, Packaging};
use sdrkit_core::publish::keys::Family;
use sdrkit_core::publish::Published;
use sdrkit_core::{
    CommandError, CommandId, CommandSet, Commands, Complex32, ConfigError, EventDef, LockState,
    Packet, Record, Results, SeriesId, Status, Values,
};
use sdrkit_graph::{
    no_values, Catalog, Component, ComponentInfo, Format, Graph, GraphBuilder, GraphError, Io,
    Item, Kept, Kind, MediaPacket, Picture, Port, RateFrom, RateRule, Timed,
};
use sdrkit_proto_bcdata::aac::AudioSpecificConfig;
use sdrkit_proto_bcdata::publish::Publisher;
use sdrkit_proto_bcdata::tags::TagSet;
use sdrkit_proto_bcdata::{datagroup, journaline, mot};
use sdrkit_proto_mpegts::publish::{SERVICE_ROW, SERVICE_SELECT_KIND};
use sdrkit_util_bits::pack::pack;
use sdrkit_util_bits::{BitOrder, Pad};
use std::collections::{BTreeMap, VecDeque};
use std::fmt::Write as _;
use std::sync::Arc;

/// One demodulated frame with its mode and FAC.
#[derive(Clone, Debug)]
pub struct DrmCells {
    /// Robustness mode.
    pub mode: Robustness,
    /// The cells.
    pub frame: Arc<CellFrame>,
    /// The FAC, when it decoded.
    pub fac: Option<Fac>,
    /// Re-synchronisations so far.
    pub resyncs: u64,
    /// Fades held so far.
    pub fades: u64,
    /// Holding through a fade now.
    pub holding: bool,
}
impl Item for DrmCells {
    const KIND: &'static str = "drm.cells";
}

/// A decoded FAC (`drm.fac`).
#[derive(Clone, Debug)]
pub struct FacItem {
    /// The block, `None` when it failed.
    pub fac: Option<Fac>,
    /// Rejected for inconsistency.
    pub rejected: bool,
    /// The mode.
    pub mode: Robustness,
}
impl Item for FacItem {
    const KIND: &'static str = "drm.fac";
}

/// A decoded SDC (`drm.sdc`).
#[derive(Clone, Debug)]
pub struct SdcItem {
    /// The SDC, `None` when it failed.
    pub sdc: Option<Sdc>,
}
impl Item for SdcItem {
    const KIND: &'static str = "drm.sdc";
}

/// One logical frame of one stream (`drm.logical_frame`).
#[derive(Clone, Debug)]
pub struct LogicalFrame {
    /// Stream number.
    pub stream: u8,
    /// Part A length.
    pub part_a: usize,
    /// Part B length.
    pub part_b: usize,
    /// The bytes.
    pub bytes: Arc<[u8]>,
    /// The multiplex frame index.
    pub mux_index: u64,
    /// Frames were lost before this one.
    pub gap_before: bool,
    /// Position in the super frame.
    pub position: usize,
    /// The mode.
    pub mode: Robustness,
}
impl Item for LogicalFrame {
    const KIND: &'static str = "drm.logical_frame";
}

/// Decoder counters, after each frame.
#[derive(Clone, Debug, Default)]
pub struct DecoderStats {
    /// FAC good.
    pub fac_good: u64,
    /// FAC bad.
    pub fac_bad: u64,
    /// FAC rejected.
    pub fac_rejected: u64,
    /// SDC good.
    pub sdc_good: u64,
    /// SDC bad.
    pub sdc_bad: u64,
    /// Multiplex frames decoded.
    pub mux_good: u64,
    /// Multiplex frames skipped (fade) or lost.
    pub mux_bad: u64,
    /// The current FAC configuration.
    pub fac: Option<Fac>,
    /// The latest SDC.
    pub sdc: Option<Sdc>,
    /// MSC constellation of the last block.
    pub msc_points: Vec<[f32; 2]>,
    /// SDC constellation.
    pub sdc_points: Vec<[f32; 2]>,
    /// FAC constellation.
    pub fac_points: Vec<[f32; 2]>,
    /// The mode.
    pub mode: Option<Robustness>,
    /// The last time.
    pub time: f64,
}
impl Item for DecoderStats {
    const KIND: &'static str = "drm.stats";
}

/// An audio frame out of the super frame splitter.
#[derive(Clone, Debug)]
pub struct AudioFrameItem {
    /// The bytes; `None` for a frame lost (CRC, unreadable super frame).
    pub bytes: Option<Arc<[u8]>>,
    /// The service's short id.
    pub short_id: u8,
    /// Gap before (frames lost).
    pub gap: bool,
}
impl Item for AudioFrameItem {
    const KIND: &'static str = "drm.audio_frame";
}

/// The text message of the playing service (`drm.text`).
#[derive(Clone, Debug)]
pub struct TextItem {
    /// The cleaned text.
    pub text: String,
    /// Segments good.
    pub good: u64,
    /// Segments bad.
    pub bad: u64,
}
impl Item for TextItem {
    const KIND: &'static str = "drm.text";
    fn event_def() -> Option<&'static EventDef> {
        Some(res::TEXT_CHANGED.def())
    }
    fn write_fields(&self, r: &mut Record) {
        res::DrmTextEvent {
            text: &self.text,
            family: Family::Broadcast,
        }
        .write(r);
    }
}

/// One data unit of a packet-mode service (`msc.data_group`).
#[derive(Clone, Debug)]
pub struct DataUnit {
    /// Short id.
    pub short_id: u8,
    /// The DAB user application type.
    pub app_type: Option<u16>,
    /// Extended header length (Journaline).
    pub ext_len: usize,
    /// The bytes.
    pub bytes: Arc<[u8]>,
}
impl Item for DataUnit {
    const KIND: &'static str = "drm.data_unit";
}

/// Packet counters per service.
#[derive(Clone, Debug)]
pub struct PacketStatsItem {
    /// Short id.
    pub short_id: u8,
    /// Counters.
    pub stats: PacketStats,
    /// TPEG frames counted.
    pub tpeg_frames: u64,
}
impl Item for PacketStatsItem {
    const KIND: &'static str = "drm.packet_stats";
}

/// A completed MOT object (`mot.object`).
#[derive(Clone, Debug)]
pub struct MotItem {
    /// Short id of the service.
    pub short_id: u8,
    /// The object.
    pub object: MotObject,
    /// Times received.
    pub times: u64,
}
impl Item for MotItem {
    const KIND: &'static str = "drm.object";
    fn event_def() -> Option<&'static EventDef> {
        Some(res::OBJECT_RECEIVED.def())
    }
    fn write_fields(&self, r: &mut Record) {
        res::DrmObjectEvent {
            transport_id: i64::from(self.object.transport_id),
            text: &self.object.name,
            content_type: &self.object.content_type_text(),
            size: self.object.body.len() as i64,
            family: Family::Broadcast,
        }
        .write(r);
    }
}

/// A Journaline object (`journaline.page`).
#[derive(Clone, Debug)]
pub struct PageItem {
    /// Short id.
    pub short_id: u8,
    /// The object.
    pub page: journaline::Object,
}
impl Item for PageItem {
    const KIND: &'static str = "drm.page";
}

/// The codec of a MOT image of content subtype `subtype` (EN 301 234 clause 6.2.3: 1 is JFIF, 3 is PNG, 0 is GIF).
fn image_codec(subtype: u16) -> Codec {
    match subtype {
        1 => Codec::Jpeg,
        3 => Codec::Png,
        _ => Codec::Gif,
    }
}

/// A port for typed items, which have no rate.
fn item_out<T: Item>(name: &str) -> Port {
    Port::typed::<T>(name).rate_from(RateFrom::Fixed(0.0))
}

// ---------------------------------------------------------------------------------------------------------------------------
// Lanes

/// The front end (`broadcast.drm_lanes`, iq -> `lane48`, `lane192`): channel centring, then the 48 kHz lane (±22 kHz) and, at
/// 100 ksps and above, the 192 kHz lane (±60 kHz) for mode E. Below 100 ksps the 192 kHz lane carries no samples.
pub struct Lanes {
    r48: Option<Resampler<Complex32>>,
    r192: Option<Resampler<Complex32>>,
    in_rate: f64,
    offset: f64,
    shift: f64,
    nco: Option<sdrkit_algo_dsp::nco::Nco>,
    nco192: Option<sdrkit_algo_dsp::nco::Nco>,
    scratch: Vec<Complex32>,
    // centring search
    fft: Option<Fft>,
    acc: Vec<f32>,
    frames: usize,
    fill: Vec<Complex32>,
}

impl Default for Lanes {
    fn default() -> Self {
        Lanes {
            r48: None,
            r192: None,
            in_rate: 0.0,
            offset: 0.0,
            shift: 0.0,
            nco: None,
            nco192: None,
            scratch: Vec::new(),
            fft: None,
            acc: Vec::new(),
            frames: 0,
            fill: Vec::new(),
        }
    }
}

const CENTRE_FFT: usize = 4096;

impl Lanes {
    fn centring(&mut self, x: &[Complex32]) {
        // Only a rate wide enough to hold a channel beyond ±24 kHz is searched.
        if self.in_rate < 100e3 {
            return;
        }
        for &s in x {
            self.fill.push(s);
            if self.fill.len() == CENTRE_FFT {
                let fft = self.fft.get_or_insert_with(|| Fft::forward(CENTRE_FFT));
                fft.process(&mut self.fill);
                if self.acc.len() != CENTRE_FFT {
                    self.acc = vec![0.0; CENTRE_FFT];
                }
                for (a, b) in self.acc.iter_mut().zip(&self.fill) {
                    *a += b.norm_sqr();
                }
                self.fill.clear();
                self.frames += 1;
                if self.frames == 32 {
                    self.decide();
                    self.frames = 0;
                    self.acc.iter_mut().for_each(|a| *a = 0.0);
                }
            }
        }
    }
    fn decide(&mut self) {
        let n = CENTRE_FFT;
        let bin = self.in_rate / n as f64;
        let w = ((10e3 / bin).round() as usize).max(3);
        let mut sorted = self.acc.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let median = sorted[n / 2].max(1e-30);
        // The 10 kHz window of most energy (DC bins excluded).
        let mut best = (0.0f32, 0i64);
        let mut run = 0.0f32;
        let shifted: Vec<f32> = (0..n).map(|i| self.acc[(i + n / 2) % n]).collect();
        for i in 0..n {
            run += shifted[i];
            if i >= w {
                run -= shifted[i - w];
            }
            if i + 1 >= w {
                let centre = i as i64 + 1 - w as i64 / 2 - 1 - n as i64 / 2;
                if run > best.0 {
                    best = (run, centre);
                }
            }
        }
        // The search sees the input as it comes (centred on the radio's centre); the lane is centred on the tuned frequency plus
        // the shift.
        let f = best.1 as f64 * bin;
        let dense = best.0 / w as f32 / median;
        if dense > 10.0 && (f - self.offset - self.shift).abs() > 24e3 {
            self.shift = f - self.offset;
            let mix = -(self.offset + self.shift);
            match &mut self.nco {
                Some(o) => o.set_freq(mix),
                None => self.nco = Some(sdrkit_algo_dsp::nco::Nco::new(self.in_rate, mix)),
            }
        }
    }
    /// The shift the centring applied, Hz.
    pub fn shift(&self) -> f64 {
        self.shift
    }
}

impl Component for Lanes {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "broadcast.drm_lanes",
            vec![Port::iq("in")],
            vec![
                Port::iq("lane48").rate_from(RateFrom::Fixed(48e3)),
                Port::iq("lane192").rate_from(RateFrom::Fixed(192e3)),
            ],
        )
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let f = &inputs[0];
        if f.rate < 48e3 {
            return Err(ConfigError::RateTooLow {
                need: 48e3,
                got: f.rate,
            });
        }
        self.in_rate = f.rate;
        self.offset = f.tuned_hz - f.centre_hz;
        self.shift = 0.0;
        self.frames = 0;
        self.fill.clear();
        self.acc.clear();
        self.nco =
            (self.offset != 0.0).then(|| sdrkit_algo_dsp::nco::Nco::new(f.rate, -self.offset));
        self.nco192 =
            (self.offset != 0.0).then(|| sdrkit_algo_dsp::nco::Nco::new(f.rate, -self.offset));
        let mk = |out: f64, pass: f64| -> Result<Option<Resampler<Complex32>>, ConfigError> {
            if (f.rate - out).abs() < 1e-9 {
                return Ok(None);
            }
            let lower = f.rate.min(out);
            Ok(Some(Resampler::with_quality(
                f.rate,
                out,
                (pass / lower).min(0.45),
                70.0,
            )?))
        };
        self.r48 = mk(48e3, 22e3)?;
        self.r192 = if f.rate >= 100e3 {
            mk(192e3, 60e3)?
        } else {
            None
        };
        let mut a = f.clone();
        a.rate = 48e3;
        a.centre_hz = f.tuned_hz;
        a.bandwidth_hz = 44e3;
        let mut b = a.clone();
        b.rate = 192e3;
        b.bandwidth_hz = 120e3;
        Ok(vec![a, b])
    }
    fn process(&mut self, io: &mut Io) {
        let x = io.iq(0);
        let mut s = std::mem::take(&mut self.scratch);
        // The DRM+ lane takes the input as it is (a 100 kHz channel is not moved), shifted to the tuned frequency only.
        if self.in_rate >= 100e3 {
            s.clear();
            s.extend_from_slice(x);
            if let Some(n) = &mut self.nco192 {
                n.mix_in_place(&mut s);
            }
            match &mut self.r192 {
                Some(r) => r.process(&s, io.iq_out(1)),
                None => io.iq_out(1).extend_from_slice(&s),
            }
        }
        s.clear();
        s.extend_from_slice(x);
        if let Some(n) = &mut self.nco {
            n.mix_in_place(&mut s);
        }
        self.centring(x);
        match &mut self.r48 {
            Some(r) => r.process(&s, io.iq_out(0)),
            None => io.iq_out(0).extend_from_slice(&s),
        }
        self.scratch = s;
    }
    fn values(&self) -> Values {
        no_values()
    }
}

// ---------------------------------------------------------------------------------------------------------------------------
// OFDM

/// Decodes the FAC of a frame.
pub fn decode_fac(
    mode: Robustness,
    frame: &CellFrame,
    mlc: &mut Mlc,
    map: &[usize],
) -> Option<Fac> {
    let z: Vec<Complex32> = map.iter().map(|&i| frame.z[i]).collect();
    let w: Vec<f32> = map.iter().map(|&i| frame.w[i]).collect();
    let mut bits = Vec::new();
    mlc.decode(&z, &w, 1, &mut bits);
    disperse(&mut bits);
    Fac::from_bits(&bits, mode)
}

struct ModeLane {
    mode: Robustness,
    demod: FrameDemodulator,
    fac_mlc: Option<Mlc>,
    fac_map: Vec<usize>,
}

/// The OFDM acquisition and tracker (`broadcast.drm_ofdm`, lane48 + lane192 -> typed `drm.cells`): searches every mode at once, follows the
/// one that locks, checks the lock with the FAC, holds through fades, and starts again when the lock is wrong.
pub struct DrmOfdm {
    lanes: Vec<ModeLane>,
    active: Option<usize>,
    frames_since_lock: u64,
    fac_ever: bool,
    fac_fail_run: u32,
    fade_frames: u32,
    snr_s: f32,
    holding: bool,
    fades: u64,
    resyncs: u64,
    results: Results,
}

impl Default for DrmOfdm {
    fn default() -> Self {
        let lanes = Robustness::ALL
            .iter()
            .map(|&m| {
                let map = CellMap::new(m, m.usual_occupancy()).expect("usual occupancy");
                ModeLane {
                    mode: m,
                    demod: FrameDemodulator::new(search_layout(m)),
                    fac_mlc: Mlc::fac(fac_rate(m), m.fac_bits(), m.fac_cells()).ok(),
                    fac_map: map.fac,
                }
            })
            .collect();
        DrmOfdm {
            lanes,
            active: None,
            frames_since_lock: 0,
            fac_ever: false,
            fac_fail_run: 0,
            fade_frames: 0,
            snr_s: 0.0,
            holding: false,
            fades: 0,
            resyncs: 0,
            results: Results::default(),
        }
    }
}

impl DrmOfdm {
    fn restart(&mut self, why: &str, block: bool) {
        if let Some(a) = self.active {
            if block {
                self.lanes[a].demod.block_current(6.0);
                let m = !self.lanes[a].demod.mirrored();
                self.lanes[a].demod.set_mirror(m);
            } else {
                self.lanes[a].demod.restart();
            }
        }
        for l in &mut self.lanes {
            l.demod.restart();
        }
        self.active = None;
        self.frames_since_lock = 0;
        self.fac_ever = false;
        self.fac_fail_run = 0;
        self.results.log(format!("search again: {why}"));
    }
}

impl Component for DrmOfdm {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "broadcast.drm_ofdm",
            vec![
                Port::iq("lane48").rate(RateRule::Exact(48e3)),
                Port::iq("lane192").rate(RateRule::Exact(192e3)),
            ],
            vec![Port::typed::<DrmCells>("cells").rate_from(RateFrom::Fixed(0.0))],
        )
    }
    fn configure(&mut self, _inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        for l in &mut self.lanes {
            l.demod.reset();
        }
        self.active = None;
        self.frames_since_lock = 0;
        self.fac_ever = false;
        Ok(vec![Format::new(Kind::typed::<DrmCells>(), 0.0)])
    }
    fn process(&mut self, io: &mut Io) {
        let a = io.iq(0);
        let b = io.iq(1);
        let mut out: Vec<DrmCells> = Vec::new();
        for i in 0..self.lanes.len() {
            if self.active.is_some_and(|x| x != i) {
                continue;
            }
            let is_e = self.lanes[i].mode == Robustness::E;
            if is_e && b.is_empty() {
                continue;
            }
            self.lanes[i].demod.push(if is_e { b } else { a });
            loop {
                if self.active.is_some_and(|x| x != i) {
                    break;
                }
                let Some(f) = self.lanes[i].demod.next_frame() else {
                    break;
                };
                if self.active.is_none() {
                    self.active = Some(i);
                    self.frames_since_lock = 0;
                    self.fac_ever = false;
                    self.fac_fail_run = 0;
                    if f.first && self.lanes[i].demod.locks > 1 {
                        self.resyncs += 1;
                    }
                    // The other lanes start afresh when next searched.
                    for (j, l) in self.lanes.iter_mut().enumerate() {
                        if j != i {
                            l.demod.reset();
                        }
                    }
                }
                let lane = &mut self.lanes[i];
                let fac = lane
                    .fac_mlc
                    .as_mut()
                    .and_then(|mlc| decode_fac(lane.mode, &f, mlc, &lane.fac_map));
                self.frames_since_lock += 1;
                // Fades: SNR smoothed over about three symbols' worth of frames.
                self.snr_s += 0.5 * (f.snr_db - self.snr_s);
                if !self.holding && self.snr_s < -1.0 && self.fac_ever {
                    self.holding = true;
                    self.fades += 1;
                    self.fade_frames = 0;
                } else if self.holding && self.snr_s > 1.5 {
                    self.holding = false;
                }
                if self.holding {
                    self.fade_frames += 1;
                }
                lane.demod.set_hold(self.holding && self.fade_frames < 6);
                match &fac {
                    Some(_) => {
                        self.fac_ever = true;
                        self.fac_fail_run = 0;
                    }
                    None => self.fac_fail_run += 1,
                }
                let mode = lane.mode;
                out.push(DrmCells {
                    mode,
                    frame: f,
                    fac,
                    resyncs: self.resyncs,
                    fades: self.fades,
                    holding: self.holding,
                });
                let limit = if mode == Robustness::E { 42 } else { 14 };
                if !self.fac_ever && self.frames_since_lock >= limit {
                    self.restart("the FAC cannot be decoded", true);
                    break;
                }
                let fail_limit = if self.holding { 60 } else { 12 };
                if self.fac_ever && self.fac_fail_run >= fail_limit {
                    self.restart("the FAC failed with the signal present", false);
                    break;
                }
            }
        }
        let o = io.items_out::<DrmCells>(0);
        for c in out {
            o.push(Timed {
                time: c.frame.time,
                value: c,
            });
        }
    }
    /// The lock, the lanes and the counts of fades and resyncs start again.
    fn forget(&mut self, _kept: &Kept) {
        for l in &mut self.lanes {
            l.demod.reset();
        }
        self.active = None;
        self.frames_since_lock = 0;
        self.fac_ever = false;
        self.holding = false;
        self.fades = 0;
        self.resyncs = 0;
    }
    fn values(&self) -> Values {
        no_values()
    }
    fn commands(&self) -> &Commands {
        ClearCommand::commands()
    }
    fn invoke(&mut self, id: CommandId, _args: &Values) -> Result<Option<Record>, CommandError> {
        match ClearCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            ClearCommand::Clear => {
                for l in &mut self.lanes {
                    l.demod.reset();
                }
                self.active = None;
                self.frames_since_lock = 0;
                self.fac_ever = false;
                self.holding = false;
            }
        }
        Ok(None)
    }
    fn results(&self) -> Option<&Results> {
        (!self.results.log_lines().is_empty()).then_some(&self.results)
    }
}

// ---------------------------------------------------------------------------------------------------------------------------
// Decoder

struct Config {
    fac: Fac,
    mode: Robustness,
    map: CellMap,
    sdc_mlc: Option<Mlc>,
    /// The MSC code and the multiplex description it was built for (protection A and B, bytes of part A).
    msc_mlc: Option<(Mlc, (u8, u8, usize))>,
    deint: CellDeinterleaver,
    nmux: usize,
}

/// The MSC code of a multiplex description (clause 7.2.1.1): `N1` cells for the bytes of part A at protection A, the rest at
/// protection B. `None` for a combination the standard does not have.
pub fn msc_code(
    mode: Robustness,
    msc_mode: u8,
    prot_a: u8,
    prot_b: u8,
    bytes_a: usize,
    nmux: usize,
) -> Option<Mlc> {
    let q = msc_qam(mode, msc_mode);
    let (rb, _) = level_rates(mode, q, prot_b)?;
    if bytes_a == 0 {
        return Mlc::eep(q, rb, nmux).ok();
    }
    let (ra, lcm) = level_rates(mode, q, prot_a)?;
    let n1 = higher_part_cells(bytes_a, ra, lcm);
    if n1 + 20 > nmux {
        return None;
    }
    Mlc::new(q, ra, rb, n1, nmux - n1).ok()
}

/// The FAC, SDC and MSC decoder (`broadcast.drm_decoder`, typed `drm.cells` -> `fac`, `sdc`, `frames` (`drm.logical_frame`), `stats`).
pub struct DrmDecoder {
    cfg: Option<Config>,
    pending: Option<Fac>,
    stats: DecoderStats,
    fi: Option<usize>,
    sf_cells: Vec<(Complex32, f32)>,
    sf_used: usize,
    mux_global: u64,
    good_weight: f32,
    gap: bool,
    streams: Vec<(u16, u16)>,
    next_streams: Option<Vec<(u16, u16)>>,
    position: usize,
}

impl Default for DrmDecoder {
    fn default() -> Self {
        DrmDecoder {
            cfg: None,
            pending: None,
            stats: DecoderStats::default(),
            fi: None,
            sf_cells: Vec::new(),
            sf_used: 0,
            mux_global: 0,
            good_weight: 0.0,
            gap: true,
            streams: Vec::new(),
            next_streams: None,
            position: 0,
        }
    }
}

fn points(z: &[Complex32], max: usize) -> Vec<[f32; 2]> {
    let step = (z.len() / max.max(1)).max(1);
    z.iter()
        .step_by(step)
        .take(max)
        .map(|c| [c.re, c.im])
        .collect()
}

impl Config {
    fn deint_depth(&self) -> usize {
        if self.fac.short_interleaver {
            1
        } else {
            self.mode.long_depth()
        }
    }
}

impl DrmDecoder {
    fn setup(&mut self, fac: &Fac, mode: Robustness) -> bool {
        let Some(map) = CellMap::new(mode, fac.occupancy) else {
            return false;
        };
        let sq = sdc_qam(mode, fac.sdc_mode);
        let nmux = map.mux_cells();
        let depth = if fac.short_interleaver {
            1
        } else {
            mode.long_depth()
        };
        self.cfg = Some(Config {
            fac: fac.clone(),
            mode,
            sdc_mlc: Mlc::eep(sq, sdc_rates(mode, fac.sdc_mode), map.sdc.len()).ok(),
            msc_mlc: None,
            deint: CellDeinterleaver::new(nmux, depth),
            nmux,
            map,
        });
        self.sf_cells.clear();
        self.sf_used = 0;
        self.gap = true;
        true
    }
    /// Protection A, protection B and the bytes of part A of the current multiplex description.
    fn multiplex(&self) -> Option<(u8, u8, usize)> {
        self.stats.sdc.as_ref().and_then(|s| {
            s.entities.iter().find_map(|e| match e {
                Entity::Multiplex {
                    next: false,
                    prot_a,
                    prot_b,
                    streams,
                } => Some((
                    *prot_a,
                    *prot_b,
                    streams.iter().map(|&(a, _)| a as usize).sum(),
                )),
                _ => None,
            })
        })
    }

    fn take_frame(
        &mut self,
        c: &DrmCells,
        fac_out: &mut Vec<FacItem>,
        sdc_out: &mut Vec<SdcItem>,
        frames: &mut Vec<LogicalFrame>,
    ) {
        let mode = c.mode;
        let nf = mode.frames_per_superframe();
        if c.frame.first {
            self.fi = None;
            self.gap = true;
            if let Some(cfg) = &mut self.cfg {
                cfg.deint.clear();
            }
            self.sf_cells.clear();
        }
        // FAC with its consistency rules.
        let mut rejected = false;
        match &c.fac {
            Some(f) => {
                let same = self
                    .cfg
                    .as_ref()
                    .is_some_and(|cfg| cfg.fac.config_key() == f.config_key() && cfg.mode == mode);
                if same {
                    self.stats.fac_good += 1;
                    if let Some(cfg) = &mut self.cfg {
                        cfg.fac = f.clone();
                    }
                } else if self.cfg.is_none()
                    || self
                        .pending
                        .as_ref()
                        .is_some_and(|p| p.config_key() == f.config_key())
                {
                    self.stats.fac_good += 1;
                    self.setup(f, mode);
                    self.pending = None;
                } else {
                    self.stats.fac_rejected += 1;
                    rejected = true;
                    self.pending = Some(f.clone());
                }
                let fi = match f.identity {
                    0 | 3 => 0,
                    2 => nf - 1,
                    _ => match self.fi {
                        Some(p) if p + 1 < nf - 1 => p + 1,
                        _ => 1,
                    },
                };
                if !rejected {
                    self.fi = Some(fi);
                } else {
                    self.fi = self.fi.map(|p| (p + 1) % nf);
                }
            }
            None => {
                self.stats.fac_bad += 1;
                self.fi = self.fi.map(|p| (p + 1) % nf);
            }
        }
        fac_out.push(FacItem {
            fac: c.fac.clone(),
            rejected,
            mode,
        });
        self.stats.mode = Some(mode);
        self.stats.time = c.frame.time;
        if let Some(cfg) = &self.cfg {
            self.stats.fac = Some(cfg.fac.clone());
            self.stats.fac_points = points(
                &cfg.map
                    .fac
                    .iter()
                    .map(|&i| c.frame.z[i])
                    .collect::<Vec<_>>(),
                1024,
            );
        }
        let (Some(fi), Some(_)) = (self.fi, self.cfg.as_ref()) else {
            return;
        };
        let mux_now = self.multiplex();
        let Some(cfg) = self.cfg.as_mut() else {
            return;
        };
        let f = &c.frame;
        // Fade skipping.
        let mean_w =
            |idx: &[usize]| idx.iter().map(|&i| f.w[i]).sum::<f32>() / idx.len().max(1) as f32;
        if fi == 0 {
            if self.sf_used < self.sf_cells.len()
                || (!self.sf_cells.is_empty() && self.sf_used == 0)
            {
                self.gap = true;
            }
            self.sf_cells.clear();
            self.sf_used = 0;
            self.position = 0;
            // SDC.
            let mw = mean_w(&cfg.map.sdc);
            let z: Vec<Complex32> = cfg.map.sdc.iter().map(|&i| f.z[i]).collect();
            let w: Vec<f32> = cfg.map.sdc.iter().map(|&i| f.w[i]).collect();
            self.stats.sdc_points = points(&z, 1024);
            let sdc = match &mut cfg.sdc_mlc {
                Some(m) if !(mw < 0.03 * self.good_weight && mw < 2.0) => {
                    let mut bits = Vec::new();
                    m.decode(&z, &w, 2, &mut bits);
                    disperse(&mut bits);
                    Sdc::from_bits(&bits, sdc_data_len(m.info_bits()))
                }
                _ => None,
            };
            match &sdc {
                Some(s) => {
                    self.stats.sdc_good += 1;
                    for e in &s.entities {
                        if let Entity::Multiplex { next, streams, .. } = e {
                            if *next {
                                self.next_streams = Some(streams.clone());
                            } else {
                                self.streams.clone_from(streams);
                            }
                        }
                    }
                    self.stats.sdc = Some(s.clone());
                }
                None => self.stats.sdc_bad += 1,
            }
            sdc_out.push(SdcItem { sdc });
        } else if self.sf_cells.is_empty() && self.sf_used == 0 && fi != 0 {
            // Joined mid super frame: wait for the next one.
            return;
        }
        for &i in &cfg.map.msc[fi] {
            self.sf_cells.push((f.z[i], f.w[i]));
        }
        // The MSC code follows the multiplex description of the SDC.
        if let Some(mx) = mux_now {
            if cfg.msc_mlc.as_ref().is_none_or(|(_, k)| *k != mx) {
                cfg.msc_mlc =
                    msc_code(mode, cfg.fac.msc_mode, mx.0, mx.1, mx.2, cfg.nmux).map(|m| (m, mx));
            }
        }
        // Multiplex frames that are complete.
        let nm = mode.frames_per_superframe();
        while self.sf_used / cfg.nmux < nm && self.sf_cells.len() >= self.sf_used + cfg.nmux {
            let cells = &self.sf_cells[self.sf_used..self.sf_used + cfg.nmux];
            let chunk: (Vec<Complex32>, Vec<f32>) = cells.iter().copied().unzip();
            self.sf_used += cfg.nmux;
            let n = self.mux_global;
            self.mux_global += 1;
            let d = cfg.deint_depth();
            let done = n.wrapping_sub(d as u64 - 1);
            let Some((z, w)) = cfg.deint.push(n, &chunk.0, &chunk.1) else {
                // Waiting, or the pre-roll after a lock.
                self.position += 1;
                continue;
            };
            let mw = w.iter().sum::<f32>() / w.len() as f32;
            if mw < 0.03 * self.good_weight && mw < 2.0 {
                self.stats.mux_bad += 1;
                self.gap = true;
                self.position += 1;
                continue;
            }
            self.good_weight += 0.1 * (mw - self.good_weight);
            self.stats.msc_points = points(&z, 1024);
            let Some((mlc, _)) = cfg.msc_mlc.as_mut() else {
                self.stats.mux_bad += 1;
                self.position += 1;
                continue;
            };
            let mut bits = Vec::new();
            mlc.decode(&z, &w, 2, &mut bits);
            disperse(&mut bits);
            let mut bytes = Vec::with_capacity(bits.len() / 8);
            pack(&bits, BitOrder::Msb, Pad::Drop, &mut bytes);
            self.stats.mux_good += 1;
            let mut off = 0usize;
            for (s, &(a, b)) in self.streams.iter().enumerate() {
                let len = a as usize + b as usize;
                if off + len > bytes.len() {
                    break;
                }
                frames.push(LogicalFrame {
                    stream: s as u8,
                    part_a: a as usize,
                    part_b: b as usize,
                    bytes: Arc::from(&bytes[off..off + len]),
                    mux_index: done,
                    gap_before: self.gap,
                    position: (self.position + nm * d - (d - 1)) % nm,
                    mode,
                });
                off += len;
            }
            self.gap = false;
            self.position += 1;
        }
    }
}

impl Component for DrmDecoder {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "broadcast.drm_decoder",
            vec![Port::typed::<DrmCells>("cells")],
            vec![
                item_out::<FacItem>("fac"),
                item_out::<SdcItem>("sdc"),
                item_out::<LogicalFrame>("frames"),
                item_out::<DecoderStats>("stats"),
            ],
        )
    }
    fn configure(&mut self, _inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let stats = std::mem::take(&mut self.stats);
        *self = DrmDecoder::default();
        self.stats.fac_good = stats.fac_good;
        self.stats.fac_bad = stats.fac_bad;
        Ok(vec![
            Format::new(Kind::typed::<FacItem>(), 0.0),
            Format::new(Kind::typed::<SdcItem>(), 0.0),
            Format::new(Kind::typed::<LogicalFrame>(), 0.0),
            Format::new(Kind::typed::<DecoderStats>(), 0.0),
        ])
    }
    fn process(&mut self, io: &mut Io) {
        let items = io.items::<DrmCells>(0);
        let (mut f, mut s, mut l) = (Vec::new(), Vec::new(), Vec::new());
        let mut st = Vec::new();
        for it in items {
            self.take_frame(&it.value, &mut f, &mut s, &mut l);
            st.push((it.time, self.stats.clone()));
        }
        let t = items.last().map(|i| i.time).unwrap_or(0.0);
        for v in f {
            io.items_out::<FacItem>(0).push(Timed { time: t, value: v });
        }
        for v in s {
            io.items_out::<SdcItem>(1).push(Timed { time: t, value: v });
        }
        for v in l {
            io.items_out::<LogicalFrame>(2)
                .push(Timed { time: t, value: v });
        }
        for (time, v) in st {
            io.items_out::<DecoderStats>(3)
                .push(Timed { time, value: v });
        }
    }
    /// The configuration of the multiplex and the counts start again.
    fn forget(&mut self, _kept: &Kept) {
        *self = DrmDecoder::default();
    }
    fn values(&self) -> Values {
        no_values()
    }
    fn commands(&self) -> &Commands {
        ClearCommand::commands()
    }
    fn invoke(&mut self, id: CommandId, _args: &Values) -> Result<Option<Record>, CommandError> {
        match ClearCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            ClearCommand::Clear => {
                self.cfg = None;
                self.sf_cells.clear();
                self.fi = None;
                self.gap = true;
            }
        }
        Ok(None)
    }
}

// ---------------------------------------------------------------------------------------------------------------------------
// Audio

/// The AudioSpecificConfig of a DRM audio service (ISO/IEC 14496-3; ES 201 980 clause 5.3.2 for xHE-AAC): AAC LC with
/// 960-sample frames, SBR (object type 5) and PS (29) signalled explicitly, or xHE-AAC (42) with the SDC's static configuration
/// appended. Empty for a sampling rate the configuration cannot state.
pub fn audio_specific_config(a: &AudioInfo) -> Vec<u8> {
    let rate = a.sample_rate();
    let channels = if a.mode == 2 { 2 } else { 1 };
    let mut out = Vec::new();
    let asc = AudioSpecificConfig {
        object_type: if a.coding == 3 { 42 } else { 2 },
        sample_rate: rate,
        channels,
        sbr_rate: (a.coding != 3 && a.sbr).then_some(2 * rate),
        ps: a.coding != 3 && a.sbr && a.mode == 1,
        frame_960: true,
    };
    if asc.write(&mut out).is_err() {
        return Vec::new();
    }
    if a.coding == 3 {
        out.extend_from_slice(&a.config);
    }
    out
}

/// The codec text of a service ("AAC, 24 kHz, mono").
pub fn codec_text(a: &AudioInfo) -> String {
    let khz = a.sample_rate() as f64 / 1000.0;
    if a.coding == 3 {
        return format!(
            "xHE-AAC, {} kHz {}",
            khz,
            if a.mode == 2 { "stereo" } else { "mono" }
        );
    }
    let kind = match (a.sbr, a.mode) {
        (true, 1) => "AAC + SBR + PS",
        (true, _) => "AAC + SBR",
        (false, _) => "AAC",
    };
    let m = match a.mode {
        0 => "mono",
        1 => "mono core",
        _ => "stereo",
    };
    format!("{kind}, {khz} kHz, {m}")
}

sdrkit_core::commands! {
    /// The commands of [`DrmAudio`].
    pub enum AudioCommand {
        /// Plays the service whose `services` row key (its short id) is the argument `row`. "auto" plays the first audio
        /// service.
        ServiceSelect = SERVICE_SELECT_KIND.command,
    }
}

/// The audio super frame splitter (`broadcast.drm_audio`, typed `frames` + `sdc` + `fac` -> media packets of AAC or xHE-AAC,
/// typed `audio` frames, typed `text`). Plays the service that `service.select` picked, else the first audio service.
#[derive(Default)]
pub struct DrmAudio {
    service: Option<u8>,
    audio: BTreeMap<u8, AudioInfo>,
    audio_services: Vec<u8>,
    format: Option<(AudioInfo, Arc<MediaFormat>)>,
    text: TextReceiver,
    pending: Option<(u64, Vec<u8>)>,
    au: u64,
}

impl DrmAudio {
    fn playing(&self) -> Option<&AudioInfo> {
        let sid = match self.service {
            Some(s) => Some(s),
            None => self.audio_services.first().copied(),
        }?;
        self.audio.get(&sid)
    }
}

impl Component for DrmAudio {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "broadcast.drm_audio",
            vec![
                Port::typed::<LogicalFrame>("frames"),
                Port::typed::<SdcItem>("sdc"),
                Port::typed::<FacItem>("fac"),
            ],
            vec![
                item_out::<MediaPacket>("media"),
                item_out::<AudioFrameItem>("audio"),
                item_out::<TextItem>("text"),
            ],
        )
    }
    fn configure(&mut self, _inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.pending = None;
        Ok(vec![
            Format::new(Kind::typed::<MediaPacket>(), 0.0),
            Format::new(Kind::typed::<AudioFrameItem>(), 0.0),
            Format::new(Kind::typed::<TextItem>(), 0.0),
        ])
    }
    fn process(&mut self, io: &mut Io) {
        for f in io.items::<FacItem>(2) {
            if let Some(fac) = &f.value.fac {
                for s in &fac.services {
                    if !s.data && !self.audio_services.contains(&s.short_id) {
                        self.audio_services.push(s.short_id);
                        self.audio_services.sort();
                    }
                }
            }
        }
        for s in io.items::<SdcItem>(1) {
            if let Some(sdc) = &s.value.sdc {
                for e in &sdc.entities {
                    if let Entity::Audio(a) = e {
                        self.audio.insert(a.short_id, a.clone());
                    }
                }
            }
        }
        let frames = io.items::<LogicalFrame>(0);
        let mut media = Vec::new();
        let mut aus = Vec::new();
        let mut texts = Vec::new();
        for it in frames {
            let lf = &it.value;
            let Some(a) = self.playing().cloned() else {
                continue;
            };
            if lf.stream != a.stream {
                continue;
            }
            let fmt = match &self.format {
                Some((ai, f)) if *ai == a => f.clone(),
                _ => {
                    let mut f = MediaFormat::new(
                        if a.coding == 3 {
                            Codec::XheAac
                        } else {
                            Codec::Aac
                        },
                        Packaging::Raw,
                    );
                    f.config = audio_specific_config(&a).into();
                    let out_rate = if a.sbr {
                        a.sample_rate() * 2
                    } else {
                        a.sample_rate()
                    };
                    f.audio = Some(AudioProps {
                        sample_rate: out_rate,
                        channels: if a.mode == 0 { 1 } else { 2 },
                    });
                    let f = Arc::new(f);
                    self.format = Some((a.clone(), f.clone()));
                    f
                }
            };
            let mut payload: &[u8] = &lf.bytes;
            if a.text && payload.len() >= 4 {
                let (p, t) = payload.split_at(payload.len() - 4);
                payload = p;
                if self.text.push(t) {
                    texts.push(TextItem {
                        text: self.text.message.clone().unwrap_or_default(),
                        good: self.text.good,
                        bad: self.text.bad,
                    });
                }
            }
            let nf = audio_frames(lf.mode);
            let sf: Option<Vec<u8>> = if lf.mode == Robustness::E {
                if lf.position % 2 == 0 {
                    self.pending = Some((lf.mux_index, payload.to_vec()));
                    None
                } else {
                    match self.pending.take() {
                        Some((m, mut first)) if m + 1 == lf.mux_index => {
                            first.extend_from_slice(payload);
                            Some(first)
                        }
                        _ => {
                            for _ in 0..nf {
                                aus.push(AudioFrameItem {
                                    bytes: None,
                                    short_id: a.short_id,
                                    gap: true,
                                });
                            }
                            None
                        }
                    }
                }
            } else {
                Some(payload.to_vec())
            };
            let Some(sf) = sf else {
                continue;
            };
            match parse_superframe(&sf, nf) {
                Some(p) => {
                    let dur = 90_000 * 960 / a.sample_rate().max(1) as i64;
                    // The CRC of a frame covers its side information and TNS bits (clause 5.4.1). Their length comes from
                    // the AAC syntax of the frame, which this receiver does not parse: the frame goes to the decoder unchecked.
                    for fr in p.frames {
                        let pts = self.au as i64 * dur;
                        self.au += 1;
                        let arc: Arc<[u8]> = fr.bytes.into();
                        media.push(MediaPacket::new(
                            Packet::new(arc.clone(), it.time).stamped(Some(pts), Some(pts), true),
                            Arc::clone(&fmt),
                        ));
                        aus.push(AudioFrameItem {
                            bytes: Some(arc),
                            short_id: a.short_id,
                            gap: lf.gap_before,
                        });
                    }
                }
                None => {
                    for _ in 0..nf {
                        aus.push(AudioFrameItem {
                            bytes: None,
                            short_id: a.short_id,
                            gap: lf.gap_before,
                        });
                    }
                }
            }
        }
        let t = frames.last().map(|f| f.time).unwrap_or(0.0);
        for m in media {
            io.items_out::<MediaPacket>(0)
                .push(Timed { time: t, value: m });
        }
        for a in aus {
            io.items_out::<AudioFrameItem>(1)
                .push(Timed { time: t, value: a });
        }
        for x in texts {
            io.items_out::<TextItem>(2)
                .push(Timed { time: t, value: x });
        }
    }
    /// The services, the text and its counts start again. The service chosen stays.
    fn forget(&mut self, _kept: &Kept) {
        *self = DrmAudio {
            service: self.service,
            ..DrmAudio::default()
        };
    }
    fn values(&self) -> Values {
        no_values()
    }
    fn commands(&self) -> &Commands {
        AudioCommand::commands()
    }
    fn invoke(&mut self, id: CommandId, args: &Values) -> Result<Option<Record>, CommandError> {
        match AudioCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            AudioCommand::ServiceSelect => {
                let row = args.arg(&SERVICE_ROW)?;
                self.service = match row {
                    "auto" => None,
                    _ => match row.parse::<u8>() {
                        Ok(sid) if sid < 4 => Some(sid),
                        _ => {
                            return Err(CommandError::NoSuchRow {
                                table: res::services_table(),
                                key: row.to_string(),
                            })
                        }
                    },
                };
                self.format = None;
                self.pending = None;
            }
        }
        Ok(None)
    }
}

// ---------------------------------------------------------------------------------------------------------------------------
// Packets, MOT, Journaline

/// The packet demultiplexer (`broadcast.drm_packets`, typed `frames` + `sdc` -> typed `units` (`msc.data_group`), `stats`).
#[derive(Default)]
pub struct DrmPackets {
    apps: BTreeMap<u8, AppInfo>,
    rx: BTreeMap<u8, PacketService>,
    tpeg: BTreeMap<u8, u64>,
    units: Vec<DataUnit>,
}

impl Component for DrmPackets {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "broadcast.drm_packets",
            vec![
                Port::typed::<LogicalFrame>("frames"),
                Port::typed::<SdcItem>("sdc"),
            ],
            vec![
                item_out::<DataUnit>("units"),
                item_out::<PacketStatsItem>("stats"),
            ],
        )
    }
    /// The packet services and their counts start again.
    fn forget(&mut self, _kept: &Kept) {
        *self = DrmPackets::default();
    }
    fn values(&self) -> Values {
        no_values()
    }
    fn configure(&mut self, _inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        Ok(vec![
            Format::new(Kind::typed::<DataUnit>(), 0.0),
            Format::new(Kind::typed::<PacketStatsItem>(), 0.0),
        ])
    }
    fn process(&mut self, io: &mut Io) {
        for s in io.items::<SdcItem>(1) {
            let Some(sdc) = &s.value.sdc else {
                continue;
            };
            for e in &sdc.entities {
                if let Entity::Application(a) = e {
                    let changed = self.apps.get(&a.short_id).is_none_or(|o| {
                        o.stream != a.stream
                            || o.packet_id != a.packet_id
                            || o.packet_len != a.packet_len
                    });
                    if changed && a.packet_mode {
                        // A change of stream, packet id or length starts the decoder over.
                        match PacketService::new(a.packet_id, a.packet_len as usize + 3) {
                            Some(p) => {
                                self.rx.insert(a.short_id, p);
                            }
                            None => {
                                self.rx.remove(&a.short_id);
                            }
                        }
                    }
                    self.apps.insert(a.short_id, a.clone());
                }
            }
        }
        let mut stats = Vec::new();
        let frames = io.items::<LogicalFrame>(0);
        for it in frames {
            for (sid, app) in &self.apps {
                if app.stream != it.value.stream || !app.packet_mode {
                    continue;
                }
                let Some(rx) = self.rx.get_mut(sid) else {
                    continue;
                };
                let ut = app.user_app_type();
                let ext_len = if ut == Some(0x44A) {
                    app.data.get(2).copied().unwrap_or(0) as usize
                } else {
                    0
                };
                let units = &mut self.units;
                let tpeg = self.tpeg.entry(*sid).or_default();
                rx.push(&it.value.bytes, &mut |u| {
                    if ut == Some(0x004) {
                        *tpeg += 1;
                    }
                    units.push(DataUnit {
                        short_id: *sid,
                        app_type: ut,
                        ext_len,
                        bytes: Arc::from(u),
                    });
                });
                stats.push(PacketStatsItem {
                    short_id: *sid,
                    stats: rx.stats,
                    tpeg_frames: *tpeg,
                });
            }
        }
        let t = frames.last().map_or(0.0, |f| f.time);
        let out = io.items_out::<DataUnit>(0);
        for u in self.units.drain(..) {
            out.push(Timed { time: t, value: u });
        }
        let out = io.items_out::<PacketStatsItem>(1);
        for s in stats {
            out.push(Timed { time: t, value: s });
        }
    }
}

/// The MOT decoder (`broadcast.drm_mot`, typed `units` -> typed `objects` (`drm.object`), `pictures`): one MOT assembler for
/// each service.
#[derive(Default)]
pub struct DrmMot {
    dec: BTreeMap<u8, mot::Assembler>,
    times: BTreeMap<(u8, u16), u64>,
}

impl Component for DrmMot {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "broadcast.drm_mot",
            vec![Port::typed::<DataUnit>("units")],
            vec![
                item_out::<MotItem>("objects"),
                item_out::<Picture>("pictures"),
            ],
        )
    }
    /// The objects being assembled and their counts start again.
    fn forget(&mut self, _kept: &Kept) {
        *self = DrmMot::default();
    }
    fn values(&self) -> Values {
        no_values()
    }
    fn configure(&mut self, _inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        Ok(vec![
            Format::new(Kind::typed::<MotItem>(), 0.0),
            Format::new(Kind::typed::<Picture>(), 0.0),
        ])
    }
    fn process(&mut self, io: &mut Io) {
        let mut objs = Vec::new();
        let mut pics = Vec::new();
        let units = io.items::<DataUnit>(0);
        for u in units {
            if !matches!(u.value.app_type, Some(0x002 | 0x003 | 0x007)) {
                continue;
            }
            let Ok(g) = datagroup::parse(&u.value.bytes) else {
                continue;
            };
            let sid = u.value.short_id;
            let d = self.dec.entry(sid).or_default();
            let times = &mut self.times;
            d.push(&g, &mut |o| {
                let t = times.entry((sid, o.transport_id)).or_default();
                *t += 1;
                let object = MotObject::from_object(&o);
                if object.content_type == 2 {
                    pics.push(Picture {
                        codec: image_codec(object.content_subtype),
                        layout: None,
                        name: (!object.name.is_empty()).then(|| object.name.clone()),
                        id: Picture::WHOLE,
                        state: sdrkit_core::PictureState::Finished,
                        data: object.body.clone(),
                    });
                }
                objs.push(MotItem {
                    short_id: sid,
                    object,
                    times: *t,
                });
            });
        }
        let t = units.last().map_or(0.0, |f| f.time);
        let out = io.items_out::<MotItem>(0);
        for o in objs {
            out.push(Timed { time: t, value: o });
        }
        let out = io.items_out::<Picture>(1);
        for p in pics {
            out.push(Timed { time: t, value: p });
        }
    }
}

/// The Journaline decoder (`broadcast.drm_journaline`, typed `units` -> typed `pages`): the NML objects in the data groups of
/// the service.
#[derive(Default)]
pub struct DrmJournaline;

impl Component for DrmJournaline {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "broadcast.drm_journaline",
            vec![Port::typed::<DataUnit>("units")],
            vec![item_out::<PageItem>("pages")],
        )
    }
    fn values(&self) -> Values {
        no_values()
    }
    fn configure(&mut self, _inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        Ok(vec![Format::new(Kind::typed::<PageItem>(), 0.0)])
    }
    fn process(&mut self, io: &mut Io) {
        let units = io.items::<DataUnit>(0);
        let pages: Vec<(f64, PageItem)> = units
            .iter()
            .filter(|u| u.value.app_type == Some(0x44A))
            .filter_map(|u| {
                let g = datagroup::parse(&u.value.bytes).ok()?;
                let page = journaline::decode(g.data, u.value.ext_len).ok()?;
                Some((
                    u.time,
                    PageItem {
                        short_id: u.value.short_id,
                        page,
                    },
                ))
            })
            .collect();
        let out = io.items_out::<PageItem>(0);
        for (t, p) in pages {
            out.push(Timed { time: t, value: p });
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------------------
// Report

/// One row of the service table.
#[derive(Clone, Debug, Default)]
pub struct ServiceRow {
    /// Short id.
    pub short_id: u8,
    /// Service id, 6 hex digits.
    pub service_id: String,
    /// Audio or data.
    pub audio: bool,
    /// Label.
    pub label: String,
    /// Language text.
    pub language: String,
    /// Country.
    pub country: String,
    /// Programme type or application.
    pub programme: String,
    /// Conditional access flagged.
    pub ca: bool,
    /// Audio coding text.
    pub codec: String,
    /// Core rate, Hz.
    pub core_rate: u32,
    /// Audio mode.
    pub audio_mode: u8,
    /// SBR.
    pub sbr: bool,
    /// Text messages.
    pub text: bool,
    /// Stream id.
    pub stream: u8,
    /// Net bit rate of its stream, bit/s.
    pub bitrate: f64,
    /// Data: application name.
    pub application: String,
    /// Data: the mode text.
    pub mode_text: String,
    /// Decoded.
    pub decoded: bool,
}

/// The latest slide of a service.
#[derive(Clone, Debug, Default)]
pub struct Slide {
    /// Order number.
    pub number: u64,
    /// Name.
    pub name: String,
    /// Category title.
    pub category: String,
    /// Category id.
    pub category_id: Option<u8>,
    /// Slide id.
    pub slide_id: Option<u8>,
    /// Click URL.
    pub click_url: String,
    /// Alternative URL.
    pub alt_url: String,
    /// Trigger time text.
    pub trigger: String,
    /// Content type text.
    pub content_type: String,
    /// File size.
    pub size: usize,
    /// Width (from the picture when it decodes).
    pub width: u32,
    /// Height.
    pub height: u32,
    /// The bytes.
    pub bytes: Option<Arc<[u8]>>,
}

/// The state that the report holds and publishes.
#[derive(Clone, Debug, Default)]
struct DrmState {
    state: u8,
    mode: Option<Robustness>,
    occupancy: Option<u8>,
    bandwidth_khz: f64,
    interleaver: String,
    msc: String,
    sdc: String,
    protection: String,
    protection_b: Option<u8>,
    streams: Vec<(u16, u16)>,
    time: String,
    level_dbfs: f32,
    snr_db: f32,
    carrier_offset_hz: f64,
    clock_ppm: f64,
    delay_ms: f64,
    sync: f32,
    quality: f64,
    audio_good: u64,
    audio_bad: u64,
    fades: u64,
    resyncs: u64,
    audio_info: String,
    text: String,
    text_good: u64,
    text_bad: u64,
    services: Vec<ServiceRow>,
    summary: String,
    notes: Vec<String>,
    signal_time_s: f64,
    first_audio_s: Option<f64>,
    holding: bool,
    mirrored: bool,
    /// The short id of the service whose audio frames came last.
    playing: Option<u8>,
    /// Its row key in the services table.
    playing_key: String,
    /// The signal time of the last audio frame.
    last_audio_s: f64,
}

/// The report (`broadcast.drm_report`, a sink of every stream): the mode's results, four times a second of signal.
pub struct DrmReport {
    published: Published,
    status: Status,
    publisher: Publisher,
    no_tags: TagSet,
    key: String,
    text_written: String,
    streams_written: Vec<(u16, u16)>,
    notes_written: usize,
    d: DrmState,
    stats: DecoderStats,
    last_cells: Option<DrmCells>,
    next_report: f64,
    objects: BTreeMap<(u8, u16), (MotItem, u64)>,
    order: u64,
    slides: BTreeMap<u8, Slide>,
    pages: journaline::Store,
    pkt: BTreeMap<u8, PacketStatsItem>,
    quality_hist: VecDeque<bool>,
    state_logged: u8,
    labels: BTreeMap<u8, String>,
    langs: BTreeMap<u8, (String, String)>,
    apps: BTreeMap<u8, AppInfo>,
    audio: BTreeMap<u8, AudioInfo>,
    fac_services: BTreeMap<u8, super::fac::FacService>,
    codec_noted: bool,
    ifft: Fft,
    ir: Vec<Complex32>,
}

/// The series of the stack, in their order in its description.
const SERIES_CHANNEL: SeriesId = SeriesId(0);
const SERIES_IMPULSE: SeriesId = SeriesId(1);
const SERIES_FAC: SeriesId = SeriesId(2);
const SERIES_SDC: SeriesId = SeriesId(3);
const SERIES_MSC: SeriesId = SeriesId(4);

impl Default for DrmReport {
    fn default() -> Self {
        DrmReport {
            published: Published::from_decl(Arc::new(crate::typed::shape(ID).results_decl())),
            status: Status::new(LockState::Searching),
            publisher: Publisher::new(),
            no_tags: TagSet::default(),
            key: String::with_capacity(16),
            text_written: String::new(),
            streams_written: Vec::new(),
            notes_written: 0,
            d: DrmState::default(),
            stats: DecoderStats::default(),
            last_cells: None,
            next_report: 0.25,
            objects: BTreeMap::new(),
            order: 0,
            slides: BTreeMap::new(),
            pages: journaline::Store::default(),
            pkt: BTreeMap::new(),
            quality_hist: VecDeque::new(),
            state_logged: 0,
            labels: BTreeMap::new(),
            langs: BTreeMap::new(),
            apps: BTreeMap::new(),
            audio: BTreeMap::new(),
            fac_services: BTreeMap::new(),
            codec_noted: false,
            ifft: Fft::inverse(256),
            ir: Vec::with_capacity(256),
        }
    }
}

/// The name of a DAB user application type (ETSI TS 101 756 table 16).
pub fn app_name(t: Option<u16>) -> String {
    match t {
        Some(0x002) => "MOT slide show".into(),
        Some(0x003) => "MOT broadcast website".into(),
        Some(0x004) => "TPEG".into(),
        Some(0x007) => "EPG".into(),
        Some(0x44A) => "Journaline".into(),
        Some(x) => format!("application 0x{x:03X}"),
        None => "DRM application".into(),
    }
}

/// The content of a Journaline object as one text: the links (`id text`), the body or the list items, one per line.
fn page_content(o: &journaline::Object) -> String {
    let mut s = String::new();
    for (id, t) in &o.links {
        s.push_str(&format!("{id} {t}\n"));
    }
    s.push_str(&o.body);
    for it in &o.items {
        if !s.is_empty() && !s.ends_with('\n') {
            s.push('\n');
        }
        s.push_str(it);
    }
    s.trim_end().to_string()
}

/// `s`, or `None` when it is empty.
fn non_empty(s: &str) -> Option<&str> {
    (!s.is_empty()).then_some(s)
}

fn kind_name(k: journaline::Kind) -> &'static str {
    match k {
        journaline::Kind::Menu => "menu",
        journaline::Kind::Text => "plain text",
        journaline::Kind::Title => "title",
        journaline::Kind::List => "list",
    }
}

impl DrmReport {
    fn update(&mut self) {
        let st = &self.stats;
        let d = &mut self.d;
        if let Some(c) = &self.last_cells {
            let f = &c.frame;
            d.mode = Some(c.mode);
            d.level_dbfs = f.level_db;
            d.snr_db = f.snr_db;
            d.carrier_offset_hz = f.cfo_hz;
            d.clock_ppm = f.ppm;
            d.delay_ms = (f.delay as f64 - c.mode.cp() as f64 / 2.0) / c.mode.rate() * 1e3;
            d.sync = f.sync;
            d.fades = c.fades;
            d.resyncs = c.resyncs;
            d.holding = c.holding;
            d.mirrored = f.mirrored;
        }
        let mode = st.mode.unwrap_or(Robustness::B);
        if let Some(fac) = &st.fac {
            d.occupancy = Some(fac.occupancy);
            d.bandwidth_khz = occupancy_khz(mode, fac.occupancy);
            d.interleaver = match (fac.short_interleaver, mode == Robustness::E) {
                (true, true) => "short (100 ms)",
                (true, false) => "short (400 ms)",
                (false, true) => "long (600 ms)",
                (false, false) => "long (2 s)",
            }
            .into();
            d.msc = msc_qam(mode, fac.msc_mode).name().into();
            if matches!(fac.msc_mode, 1 | 2) && mode != Robustness::E {
                d.msc.push_str(", hierarchical");
                let note =
                    "The signal uses a hierarchical mapping, which this stack does not decode";
                if !d.notes.iter().any(|n| n == note) {
                    d.notes.push(note.into());
                }
            }
            d.sdc = sdc_qam(mode, fac.sdc_mode).name().into();
            for s in &fac.services {
                self.fac_services.insert(s.short_id, s.clone());
            }
        }
        if let Some(sdc) = &st.sdc {
            for e in &sdc.entities {
                match e {
                    Entity::Multiplex {
                        next: false,
                        prot_a,
                        prot_b,
                        streams,
                    } => {
                        d.streams.clone_from(streams);
                        d.protection_b = Some(*prot_b);
                        let q = msc_qam(mode, st.fac.as_ref().map_or(0, |f| f.msc_mode));
                        let pct = |p: u8| {
                            level_rates(mode, q, p).map_or(0.0, |(r, _)| overall_rate_pct(r))
                        };
                        let bytes_a: u32 = streams.iter().map(|&(a, _)| a as u32).sum();
                        d.protection = if bytes_a > 0 {
                            format!(
                                "A {} ({:.0} %), B {} ({:.0} %), part A is {} bytes",
                                prot_a,
                                pct(*prot_a),
                                prot_b,
                                pct(*prot_b),
                                bytes_a
                            )
                        } else {
                            format!("{} ({:.0} %)", prot_b, pct(*prot_b))
                        };
                    }
                    Entity::Label { short_id, text } => {
                        self.labels.insert(*short_id, text.clone());
                    }
                    Entity::Time {
                        mjd,
                        hour,
                        minute,
                        offset_half_hours,
                    } => {
                        d.time = format!(
                            "{} {} UTC{}",
                            super::fac::weekday(*mjd),
                            super::fac::date_text(*mjd, *hour, *minute).unwrap_or_default(),
                            offset_half_hours
                                .map(|o| format!(", local {:+.1} h", o as f64 / 2.0))
                                .unwrap_or_default()
                        );
                    }
                    Entity::Audio(a) => {
                        self.audio.insert(a.short_id, a.clone());
                    }
                    Entity::Language {
                        short_id,
                        language,
                        country,
                    } => {
                        self.langs
                            .insert(*short_id, (language.clone(), country.clone()));
                    }
                    Entity::Application(a) => {
                        self.apps.insert(a.short_id, a.clone());
                    }
                    _ => {}
                }
            }
        }
        let frame_s = mode.frame_s();
        let mut rows = Vec::new();
        for (sid, s) in &self.fac_services {
            let a = self.audio.get(sid);
            let app = self.apps.get(sid);
            let stream = a.map(|x| x.stream).or(app.map(|x| x.stream)).unwrap_or(0);
            let bytes = d
                .streams
                .get(stream as usize)
                .map_or(0.0, |&(x, y)| x as f64 + y as f64);
            let lang = self.langs.get(sid).map(|l| l.0.clone()).unwrap_or_default();
            rows.push(ServiceRow {
                short_id: *sid,
                service_id: format!("{:06X}", s.id),
                audio: !s.data,
                label: self.labels.get(sid).cloned().unwrap_or_default(),
                language: format!(
                    "{} {}{}",
                    s.language,
                    language_name(s.language),
                    if lang.is_empty() {
                        String::new()
                    } else {
                        format!(", {lang}")
                    }
                ),
                country: self.langs.get(sid).map(|l| l.1.clone()).unwrap_or_default(),
                programme: if s.data {
                    format!("application {}", s.descriptor)
                } else {
                    format!("{} {}", s.descriptor, programme_type_name(s.descriptor))
                },
                ca: s.audio_ca || s.data_ca,
                codec: a.map(codec_text).unwrap_or_default(),
                core_rate: a.map_or(0, |x| x.sample_rate()),
                audio_mode: a.map_or(0, |x| x.mode),
                sbr: a.is_some_and(|x| x.sbr),
                text: a.is_some_and(|x| x.text),
                stream,
                bitrate: bytes * 8.0 / frame_s,
                application: if s.data {
                    app_name(app.and_then(|x| x.user_app_type()))
                } else {
                    String::new()
                },
                mode_text: app
                    .map(|x| {
                        if x.packet_mode {
                            format!(
                                "packet mode, {}, packet id {}, {} byte packets",
                                if x.data_units {
                                    "data units"
                                } else {
                                    "single packets"
                                },
                                x.packet_id,
                                x.packet_len
                            )
                        } else {
                            "stream mode data, not decoded".into()
                        }
                    })
                    .unwrap_or_default(),
                decoded: if s.data {
                    app.is_some_and(|x| {
                        x.packet_mode
                            && x.data_units
                            && matches!(
                                x.user_app_type(),
                                Some(0x002 | 0x003 | 0x007 | 0x44A | 0x004)
                            )
                    })
                } else {
                    true
                },
            });
        }
        d.services = rows;
        let have_lock = self.last_cells.is_some();
        d.state = if !have_lock {
            0
        } else if st.sdc_good > 0 && st.mux_good > 0 {
            2
        } else {
            1
        };
        let playing = match d.playing {
            Some(p) => d.services.iter().find(|s| s.short_id == p),
            None => d.services.iter().find(|s| s.audio),
        };
        d.audio_info = match playing {
            Some(s) if s.codec.starts_with("xHE") => format!(
                "{} (xHE-AAC decoder not available. Most DRM stations (AAC) play without it.)",
                s.codec
            ),
            Some(s) if !s.codec.is_empty() => format!("{}; sound is decoded by the host", s.codec),
            _ => "no audio stream".into(),
        };
        let q_good = self.quality_hist.iter().filter(|g| **g).count() as f64;
        let q_n = self.quality_hist.len().max(1) as f64;
        let snr_w = ((d.snr_db as f64 - 5.0) / 15.0).clamp(0.0, 1.0);
        d.quality = (q_good / q_n) * (0.5 + 0.5 * snr_w);
        let letter = d.mode.map_or("-", Robustness::letter);
        d.summary.clear();
        // Writing into a String cannot fail.
        let _ = match d.state {
            0 => write!(d.summary, "searching for a DRM signal"),
            1 => write!(d.summary, "mode {letter} found, reading the FAC"),
            _ if d.holding => write!(
                d.summary,
                "holding through a fade (frequency, timing and clock frozen)"
            ),
            _ => write!(d.summary, "decoding mode {letter}, {} kHz", d.bandwidth_khz),
        };
        if d.state != self.state_logged {
            let line = d.summary.clone();
            self.published.results_mut().log(line);
            self.state_logged = d.state;
        }
    }

    fn publish(&mut self) {
        self.update();
        let d = &self.d;
        let st = &self.stats;
        let p = &mut self.published;
        let framed = self.last_cells.is_some();
        let fac = st.fac.is_some();
        p.put(&res::DrmFields {
            robustness_mode: d.mode,
            occupancy: d.occupancy.map(i64::from),
            bandwidth: fac.then_some(Hertz(d.bandwidth_khz * 1e3)),
            interleaver: fac.then_some(d.interleaver.as_str()),
            msc: fac.then_some(d.msc.as_str()),
            sdc: fac.then_some(d.sdc.as_str()),
            protection: d.protection_b.map(|_| d.protection.as_str()),
            protection_b: d.protection_b.map(i64::from),
            time: non_empty(&d.time),
            level_dbfs: framed.then_some(f64::from(d.level_dbfs)),
            snr_db: framed.then_some(f64::from(d.snr_db)),
            offset_hz: framed.then_some(Hertz(d.carrier_offset_hz)),
            clock_ppm: framed.then_some(d.clock_ppm),
            delay_ms: framed.then_some(d.delay_ms),
            guard_correlation: framed.then_some(f64::from(d.sync)),
            quality: framed.then_some(d.quality),
            fac_ok: st.fac_good as i64,
            fac_bad: st.fac_bad as i64,
            sdc_ok: st.sdc_good as i64,
            sdc_bad: st.sdc_bad as i64,
            mux_ok: st.mux_good as i64,
            mux_bad: st.mux_bad as i64,
            audio_ok: d.audio_good as i64,
            audio_bad: d.audio_bad as i64,
            fades: d.fades as i64,
            resyncs: d.resyncs as i64,
            audio_state: if d.audio_good + d.audio_bad == 0 {
                res::AudioState::NoAudioStream
            } else {
                res::AudioState::NoDecoder
            },
            audio_info: &d.audio_info,
            text_ok: d.text_good as i64,
            text_bad: d.text_bad as i64,
            signal_time: Seconds(d.signal_time_s),
            first_audio: d.first_audio_s.map(Seconds),
            holding: d.holding,
            mirrored: d.mirrored,
            playing_service: d.playing.map(|_| d.playing_key.as_str()),
        });
        if self.streams_written != d.streams {
            p.clear_rows(&res::STREAMS);
            for (i, &(a, b)) in d.streams.iter().enumerate() {
                self.key.clear();
                let _ = write!(self.key, "{i}");
                let row = res::DrmStreamRow {
                    part_a: i64::from(a),
                    part_b: i64::from(b),
                };
                res::STREAMS.put(p, &self.key, &row);
            }
            self.streams_written.clone_from(&d.streams);
        }
        if self.text_written != d.text {
            self.publisher.label(p, &d.text, &self.no_tags);
            self.text_written.clone_from(&d.text);
        }
        for (i, n) in d.notes.iter().enumerate().skip(self.notes_written) {
            self.key.clear();
            let _ = write!(self.key, "{i}");
            res::NOTES.put(p, &self.key, &res::DrmNoteRow { note: n });
        }
        self.notes_written = d.notes.len();
        for s in &d.services {
            self.key.clear();
            let _ = write!(self.key, "{}", s.short_id);
            let row = res::DrmServiceRow {
                short_id: i64::from(s.short_id),
                service_id: &s.service_id,
                kind: if s.audio { "audio" } else { "data" },
                label: non_empty(&s.label),
                language: &s.language,
                country: non_empty(&s.country),
                programme: &s.programme,
                ca: s.ca,
                codec: non_empty(&s.codec),
                core_rate: (s.core_rate > 0).then_some(Hertz(f64::from(s.core_rate))),
                audio_mode: i64::from(s.audio_mode),
                sbr: s.sbr,
                text: s.text,
                stream: i64::from(s.stream),
                bit_rate_bps: s.bitrate,
                application: non_empty(&s.application),
                mode_text: non_empty(&s.mode_text),
                decoded: s.decoded,
            };
            res::SERVICES.put(p, &self.key, &row);
        }
        for ((sid, tid), (o, ord)) in &self.objects {
            let ct = o.object.content_type_text();
            let note = match &o.object.slide {
                Some(sl) if sl.category.is_some() => {
                    let (c, i) = sl.category.unwrap_or_default();
                    format!("slide {i} in category {c}")
                }
                _ if o.object.content_type == 7 || ct == "text/XML" => "XML in the clear".into(),
                _ => String::new(),
            };
            let name = if o.object.name.is_empty() {
                tid.to_string()
            } else {
                o.object.name.clone()
            };
            self.key.clear();
            let _ = write!(self.key, "{sid}:{tid}");
            let row = res::DrmObjectRow {
                short_id: i64::from(*sid),
                transport_id: i64::from(*tid),
                name: &name,
                content_type: &ct,
                size: o.object.body.len() as i64,
                received: o.times as i64,
                order: *ord as i64,
                note: non_empty(&note),
            };
            res::OBJECTS.put(p, &self.key, &row);
        }
        for (sid, s) in &self.slides {
            self.key.clear();
            let _ = write!(self.key, "{sid}");
            let row = res::DrmSlideRow {
                short_id: i64::from(*sid),
                order: s.number as i64,
                name: &s.name,
                category: non_empty(&s.category),
                category_id: s.category_id.map(i64::from),
                slide_id: s.slide_id.map(i64::from),
                click_url: non_empty(&s.click_url),
                alt_url: non_empty(&s.alt_url),
                trigger: non_empty(&s.trigger),
                content_type: &s.content_type,
                size: s.size as i64,
                width: i64::from(s.width),
                height: i64::from(s.height),
            };
            res::SLIDES.put(p, &self.key, &row);
        }
        for (id, o) in &self.pages.objects {
            self.key.clear();
            let _ = write!(self.key, "{id}");
            let content = page_content(o);
            let row = res::DrmPageRow {
                id: i64::from(*id),
                kind: kind_name(o.kind),
                title: &o.title,
                content: &content,
            };
            res::PAGES.put(p, &self.key, &row);
        }
        for (sid, x) in &self.pkt {
            let s = &x.stats;
            self.key.clear();
            let _ = write!(self.key, "{sid}");
            let row = res::DrmPacketRow {
                short_id: i64::from(*sid),
                packets: s.packets as i64,
                bad_crc: s.bad_crc as i64,
                lost: s.lost as i64,
                orphans: s.orphans as i64,
                empty: s.empty as i64,
                other_id: s.other_id as i64,
                units_ok: s.units_good as i64,
                units_dropped: s.units_dropped as i64,
                unit_bytes: s.unit_bytes as i64,
                tpeg_frames: x.tpeg_frames as i64,
            };
            res::PACKET_COUNTERS.put(p, &self.key, &row);
        }
        let st_ = &mut self.status;
        st_.lock_state = match d.state {
            0 => LockState::Searching,
            1 => LockState::Tracking,
            _ => LockState::Locked,
        };
        st_.snr_db = Some(d.snr_db);
        st_.level_dbfs = Some(d.level_dbfs);
        st_.offset_hz = Some(d.carrier_offset_hz);
        st_.summary.clone_from(&d.summary);
        let r = p.results_mut();
        r.set_status(&self.status);
        if let Some(c) = &self.last_cells {
            let h = &c.frame.h;
            let (lo, _) = c.mode.union_range();
            let occ = st.fac.as_ref().and_then(|f| c.mode.range(f.occupancy));
            r.update_series(SERIES_CHANNEL, |s| {
                s.line = true;
                s.points.clear();
                let step = (h.len() / 512).max(1);
                for (i, v) in h.iter().enumerate().step_by(step) {
                    let k = lo + i as i32;
                    if occ.is_none_or(|(a, b)| k >= a && k <= b) {
                        s.points
                            .push([k as f32, 10.0 * v.norm_sqr().max(1e-12).log10()]);
                    }
                }
                true
            });
            // The impulse response from the channel of the newest symbol.
            let buf = &mut self.ir;
            buf.clear();
            buf.resize(256, Complex32::new(0.0, 0.0));
            let n = h.len().min(256);
            buf[..n].copy_from_slice(&h[..n]);
            self.ifft.process(buf);
            let dt_ms = c.mode.nfft() as f64 / c.mode.rate() * 1e3 / 256.0;
            r.update_series(SERIES_IMPULSE, |ir| {
                ir.line = true;
                ir.points.clear();
                for i in 0..256 {
                    let j = (i + 128) % 256;
                    ir.points.push([
                        ((i as f64 - 128.0) * dt_ms) as f32,
                        10.0 * buf[j].norm_sqr().max(1e-12).log10(),
                    ]);
                }
                true
            });
        }
        for (id, pts) in [
            (SERIES_FAC, &st.fac_points),
            (SERIES_SDC, &st.sdc_points),
            (SERIES_MSC, &st.msc_points),
        ] {
            if !pts.is_empty() {
                r.update_series(id, |s| {
                    sdrkit_core::results::copy_if_changed(&mut s.points, pts)
                });
            }
        }
    }
}

impl Component for DrmReport {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::sink(
            "broadcast.drm_report",
            vec![
                Port::typed::<DrmCells>("cells"),
                Port::typed::<DecoderStats>("stats"),
                Port::typed::<AudioFrameItem>("audio"),
                Port::typed::<TextItem>("text"),
                Port::typed::<PacketStatsItem>("packets"),
                Port::typed::<MotItem>("objects"),
                Port::typed::<PageItem>("pages"),
            ],
        )
    }
    fn configure(&mut self, _inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        Ok(vec![])
    }
    fn process(&mut self, io: &mut Io) {
        let mut tmax = 0.0f64;
        for c in io.items::<DrmCells>(0) {
            tmax = tmax.max(c.time);
            self.last_cells = Some(c.value.clone());
            self.d.signal_time_s = c.time + c.value.mode.frame_s();
            self.quality_hist.push_back(c.value.fac.is_some());
            while self.quality_hist.len() > 30 {
                self.quality_hist.pop_front();
            }
        }
        for s in io.items::<DecoderStats>(1) {
            self.stats = s.value.clone();
        }
        for a in io.items::<AudioFrameItem>(2) {
            if self.d.playing != Some(a.value.short_id) {
                self.d.playing = Some(a.value.short_id);
                self.d.playing_key = a.value.short_id.to_string();
            }
            self.d.last_audio_s = self.d.signal_time_s;
            if a.value.bytes.is_some() {
                self.d.audio_good += 1;
                if self.d.first_audio_s.is_none() {
                    self.d.first_audio_s = Some(self.d.signal_time_s);
                }
            } else {
                self.d.audio_bad += 1;
            }
            self.quality_hist.push_back(a.value.bytes.is_some());
            if !self.codec_noted {
                self.codec_noted = true;
                self.published
                    .results_mut()
                    .log("AAC frames go to the host's decoder; none is part of the stack");
            }
        }
        // No audio frame for a second: no service plays (a data service is chosen, or the audio stream went).
        if self.d.signal_time_s - self.d.last_audio_s > 1.0 {
            self.d.playing = None;
        }
        for t in io.items::<TextItem>(3) {
            self.d.text.clone_from(&t.value.text);
            self.d.text_good = t.value.good;
            self.d.text_bad = t.value.bad;
        }
        for p in io.items::<PacketStatsItem>(4) {
            self.pkt.insert(p.value.short_id, p.value.clone());
        }
        for o in io.items::<MotItem>(5) {
            self.order += 1;
            let key = (o.value.short_id, o.value.object.transport_id);
            self.objects.insert(key, (o.value.clone(), self.order));
            while self.objects.len() > 96 {
                let oldest = self
                    .objects
                    .iter()
                    .min_by_key(|(_, v)| v.1)
                    .map(|(k, _)| *k);
                if let Some(k) = oldest {
                    self.objects.remove(&k);
                }
            }
            let ob = &o.value.object;
            if let Some(sl) = &ob.slide {
                let (w, hh) = sdrkit_codec_image::decode(image_codec(ob.content_subtype), &ob.body)
                    .map_or((0, 0), |r| (r.width(), r.height()));
                let s = Slide {
                    number: self.order,
                    name: if ob.name.is_empty() {
                        ob.transport_id.to_string()
                    } else {
                        ob.name.clone()
                    },
                    category: sl.category_title.clone(),
                    category_id: sl.category.map(|c| c.0),
                    slide_id: sl.category.map(|c| c.1),
                    click_url: sl.click_url.clone(),
                    alt_url: sl.alt_url.clone(),
                    trigger: sl.trigger.clone(),
                    content_type: ob.content_type_text(),
                    size: ob.body.len(),
                    width: w,
                    height: hh,
                    bytes: Some(ob.body.clone()),
                };
                self.slides.insert(o.value.short_id, s);
            }
        }
        for p in io.items::<PageItem>(6) {
            self.pages.add(p.value.page.clone());
        }
        let t = self.d.signal_time_s.max(tmax);
        if t >= self.next_report {
            while self.next_report <= t {
                self.next_report += 0.25;
            }
            self.publish();
        }
    }
    fn values(&self) -> Values {
        no_values()
    }
    fn results(&self) -> Option<&Results> {
        Some(self.published.results())
    }
    /// Everything of the multiplex on the frequency before goes: the services, the objects, the slides, the pages,
    /// the text, the counts and the measurements. The tables of `kept` stay.
    fn forget(&mut self, kept: &Kept) {
        let mut published = std::mem::replace(&mut self.published, Published::new(&[]));
        let mut publisher = std::mem::take(&mut self.publisher);
        *self = DrmReport::default();
        publisher.clear_label(&mut published);
        let shape = crate::typed::shape(ID);
        for t in &shape.tables {
            if !kept.table(t.def.key) {
                published.clear_rows(t.def);
            }
        }
        let r = published.results_mut();
        for id in [
            SERIES_CHANNEL,
            SERIES_IMPULSE,
            SERIES_FAC,
            SERIES_SDC,
            SERIES_MSC,
        ] {
            r.update_series(id, |s| {
                let changed = !s.points.is_empty();
                s.points.clear();
                changed
            });
        }
        self.published = published;
        self.publisher = publisher;
        self.publish();
    }
}

/// Service counts of the FAC's code, as text.
pub fn services_text(code: u8) -> String {
    match service_counts(code) {
        Some((a, d)) => format!("{a} audio, {d} data"),
        None => "reserved".into(),
    }
}

/// The DRM graph.
///
/// # Errors
///
/// [`GraphError`] if the graph does not build: a fault of this crate, never of the input.
pub fn graph(catalog: &Arc<Catalog>) -> Result<Graph, GraphError> {
    GraphBuilder::new()
        .add("lanes", Lanes::default())
        .add("ofdm", DrmOfdm::default())
        .add("decoder", DrmDecoder::default())
        .add("audio", DrmAudio::default())
        .add("packets", DrmPackets::default())
        .add("mot", DrmMot::default())
        .add("journaline", DrmJournaline)
        .add("report", DrmReport::default())
        .input("lanes")
        .connect("lanes.lane48", "ofdm.lane48")
        .connect("lanes.lane192", "ofdm.lane192")
        .connect("ofdm", "decoder")
        .connect("decoder.frames", "audio.frames")
        .connect("decoder.sdc", "audio.sdc")
        .connect("decoder.fac", "audio.fac")
        .connect("decoder.frames", "packets.frames")
        .connect("decoder.sdc", "packets.sdc")
        .connect("packets.units", "mot")
        .connect("packets.units", "journaline")
        .connect("ofdm", "report.cells")
        .connect("decoder.stats", "report.stats")
        .connect("audio.audio", "report.audio")
        .connect("audio.text", "report.text")
        .connect("packets.stats", "report.packets")
        .connect("mot.objects", "report.objects")
        .connect("journaline", "report.pages")
        .output("audio.media")
        .output("audio.text")
        .output("mot.objects")
        .output("mot.pictures")
        .status_from("report")
        .catalog(catalog)
        .build()
}
