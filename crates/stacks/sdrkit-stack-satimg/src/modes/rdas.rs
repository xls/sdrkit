//! Elektro-L and Arktika-M raw downlink (`rdas`, submodes `elektro_l` and `arktika_m`): BPSK at 30.72 Msym/s, NRZ-M, CADUs
//! of 1024 bytes behind the marker 1ACFFC1D with the CCSDS randomiser and no Reed-Solomon code, and in the payload of each
//! virtual channel the MSU-GS imager frames: infrared (7 channels, 3294 columns) and visible (3 channels, shown reduced 4:1).
//! A gate in front of the demodulator lets most of the input go by unread while no carrier is found.
//!
//! No public document gives this link or the MSU-GS frames: the symbol rate, the roll-off, the frame header rule of the
//! virtual channel, the MSU-GS sync word and frame layouts are not confirmed by a public document.
//!
//! Standard: CCSDS 131.0-B-5 (marker, randomiser).

use super::display::{log_lines, published, status, with_series, Histories};
use super::tables::{frame_table_fields, publish_tables};
use crate::typed::{FrameTableFields, RdasFields, FRAMES, VIRTUAL_CHANNELS};
use crate::components::gated::{GateReport, GateRules, GatedDemodComponent};
use crate::components::instruments::instrument_owner;
use crate::components::link::{item_format, item_port, DemodReport, FrontEndComponent, FrontReport};
use crate::components::pass::{
    pass_format, pass_port, PassPictures, PassSetting, PassTimer, PASS_GAP,
};
use crate::components::table::{FrameBytes, FrameTableComponent, Line, TableLink, TableReport};
use crate::components::{last, Control};
use crate::gen::{BitSource, GenError, ModParams, Modulator};
use crate::instruments::msugs::{self, IrFrame};
use crate::items::{Cadu, CADU_LEN};
use crate::psk::{LinkClass, LinkParams, Modulation, Search, SnrEstimator};
use sdrkit_algo_fec::rs::Basis;
use sdrkit_codec_image::component::{Layout, NamedView};
use sdrkit_codec_image::strip::Strip;
use sdrkit_codec_image::strip::View;
use sdrkit_core::interface::{Fields as _, Hertz};
use sdrkit_core::publish::{field, Published};
use sdrkit_core::{
    AutoGain, ConfigError, CostTable, GenConfig, Generator, LockProfile, Receiver, Results, ResultsDecl,
    Setting, SettingError, SettingId, SettingSet, SignalInfo, StackConfig, StackDescriptor,
    SubMode, Tuning, Value, Values,
};
use sdrkit_graph::{
    no_values, Applied, Catalog, Component, ComponentInfo, Format, GraphBuilder, GraphError, Io, Item,
    Port, Stage, StackGraph, Timed,
};
use sdrkit_proto_ccsds::channel::ChannelConfig;
use sdrkit_proto_ccsds::wordsync::{WordSync, WordSyncConfig};
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::sync::Arc;

/// The stack id.
pub const ID: &str = "rdas";
/// The submode of Elektro-L.
pub const ELEKTRO_L: &str = "elektro_l";
/// The submode of Arktika-M.
pub const ARKTIKA_M: &str = "arktika_m";
/// Symbols a second (not confirmed by a public document).
pub const LINK_SYMBOL_RATE: f64 = 30_720_000.0;
/// The roll-off (not confirmed by a public document).
pub const LINK_ROLL_OFF: f64 = 0.5;
/// The input rate the stack asks for.
pub const ASKED_RATE: f64 = 45e6;
/// The least input rate.
pub const MIN_RATE: f64 = 40e6;
/// Seconds of signal between reports.
pub const REPORT_S: f64 = 0.1;
/// The first payload byte of a CADU (sync word included).
pub const PAYLOAD_AT: usize = 24;
/// The virtual channels that carry MSU-GS frames: infrared, then the visible channels 1, 2 and 3.
pub const IR_VC: u8 = 4;
/// The virtual channels of the visible channels 1, 2 and 3.
pub const VIS_VCS: [u8; 3] = [2, 5, 3];
/// The instrument name of the infrared picture.
pub const IR_NAME: &str = "MSU-GS IR";
/// The instrument name of the reduced visible picture.
pub const VIS_NAME: &str = "MSU-GS visible (reduced)";

// ---------------------------------------------------------------------------------------------------------------------------------
// Settings.

/// Symbols a second.
pub const SYMBOL_RATE: Setting<field::Hz> =
    Setting::new("symbol_rate", "Symbol rate", "Hz", LINK_SYMBOL_RATE)
        .range(1e6, 60e6, 1.0)
        .help("Not confirmed by a public document.")
        .restart();
/// The roll-off of the matched filter.
pub const ROLL_OFF: Setting<field::F64> = Setting::new("roll_off", "Matched filter roll-off", "", LINK_ROLL_OFF)
    .range(0.1, 1.0, 0.01)
    .help("Not confirmed by a public document.")
    .restart();

sdrkit_core::settings! {
    /// The settings of `rdas`.
    pub enum RdasSetting {
        /// Symbols a second.
        SymbolRate = SYMBOL_RATE,
        /// The roll-off.
        RollOff = ROLL_OFF,
        /// The end of a pass.
        PassGap = PASS_GAP,
    }
}

/// The link of the settings `v`.
pub fn link(v: &Values) -> LinkParams {
    LinkParams {
        modulation: Modulation::Bpsk,
        symbol_rate: v.get(&SYMBOL_RATE),
        roll_off: v.get(&ROLL_OFF),
        class: LinkClass::Leo,
        search: Search::PowerLine {
            power: 2,
            min_ratio: 6.0,
        },
        snr: SnrEstimator::Decisions,
        bpsk_plus_is_zero: false,
        constellation: 256,
        interleaved: false,
    }
}

/// The gate: pieces of 32 768 samples, open at a line 40 times over the mean, 7 pieces let go by after noise.
pub const GATE: GateRules = GateRules {
    piece: 32_768,
    open_ratio: 40.0,
    matched: false,
    skip: 7,
    span_hz: f64::INFINITY,
};

/// The frame rules: the marker with 2 wrong bits to acquire and 8 to track, either polarity, NRZ-M, the randomiser over the
/// 1020 bytes after the marker, no Reed-Solomon code.
pub fn table(symbol_rate: f64) -> TableLink {
    TableLink {
        sync: WordSyncConfig {
            word: 0x1ACF_FC1D,
            word_bits: 32,
            frame_bits: CADU_LEN * 8,
            acquire_errors: 2,
            track_errors: 8,
            complement: true,
            max_misses: 3,
        },
        line: Line::NrzM,
        code: Some(ChannelConfig::uncoded(CADU_LEN - 4)),
        data_randomised: false,
        invert: false,
        retry_plain: false,
        llr_rate: symbol_rate,
    }
}

// ---------------------------------------------------------------------------------------------------------------------------------
// The MSU-GS component.

/// The virtual channel of a CADU by the header rule of this link: a = bits 4 to 6 of byte 5, b = bits 4 to 6 of byte 11 (bytes
/// counted from the sync word); the first of 2, 3, 4, 5 that equals a or b, else a.
pub fn virtual_channel(cadu: &[u8]) -> u8 {
    let a = (cadu[5] >> 1) & 7;
    let b = (cadu[11] >> 1) & 7;
    [2u8, 3, 4, 5]
        .into_iter()
        .find(|&v| v == a || v == b)
        .unwrap_or(a)
}

/// Counters of [`MsuGsDecoder`].
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MsuGsStats {
    /// CADUs taken.
    pub cadus: u64,
    /// CADUs of each virtual channel.
    pub per_vc: BTreeMap<u8, u64>,
    /// The virtual channel of the last CADU.
    pub last_vc: Option<u8>,
    /// MSU-GS frames found (infrared and visible).
    pub frames: u64,
    /// Infrared frames placed.
    pub ir_frames: u64,
    /// Visible frames placed in the reduced picture.
    pub vis_frames: u64,
    /// Frames whose counters were out of range.
    pub invalid: u64,
    /// Infrared lines that got a segment (the highest line plus 1).
    pub ir_lines: u32,
    /// Reduced visible lines (the highest line plus 1).
    pub vis_lines: u32,
}

/// The MSU-GS frames of the CADU payloads: one synchroniser for each virtual channel of the imager. Infrared frames become
/// strips of 183 pixels of each channel, visible frames whose line is a multiple of 4 strips of the reduced picture.
pub struct MsuGsDecoder {
    syncs: BTreeMap<u8, WordSync>,
    bits: Vec<u8>,
    found: Vec<(u8, Vec<u8>)>,
    picture: u32,
    /// The counters.
    pub stats: MsuGsStats,
}

impl Default for MsuGsDecoder {
    fn default() -> Self {
        let sync = |bytes: usize| {
            WordSync::new(WordSyncConfig {
                word: msugs::SYNC,
                word_bits: 64,
                frame_bits: bytes * 8,
                acquire_errors: 10,
                track_errors: 14,
                complement: true,
                max_misses: 3,
            })
            .unwrap_or_else(|_| unreachable!("constant rules"))
        };
        let mut syncs = BTreeMap::new();
        syncs.insert(IR_VC, sync(msugs::IR_BYTES));
        for vc in VIS_VCS {
            syncs.insert(vc, sync(msugs::VIS_BYTES));
        }
        MsuGsDecoder {
            syncs,
            bits: Vec::with_capacity(8 * (CADU_LEN - PAYLOAD_AT)),
            found: Vec::new(),
            picture: 0,
            stats: MsuGsStats::default(),
        }
    }
}

impl MsuGsDecoder {
    /// Starts new pictures.
    pub fn new_picture(&mut self) {
        self.picture += 1;
        self.stats.ir_lines = 0;
        self.stats.vis_lines = 0;
    }

    /// Takes one CADU (sync word first, derandomised) and appends the strips of the frames it completes to `out`.
    pub fn push_cadu(&mut self, cadu: &[u8], out: &mut Vec<Strip>) {
        if cadu.len() < CADU_LEN {
            return;
        }
        let vc = virtual_channel(cadu);
        self.stats.cadus += 1;
        self.stats.last_vc = Some(vc);
        *self.stats.per_vc.entry(vc).or_insert(0) += 1;
        let Some(sync) = self.syncs.get_mut(&vc) else {
            return;
        };
        self.bits.clear();
        for &b in &cadu[PAYLOAD_AT..CADU_LEN] {
            self.bits.extend((0..8).rev().map(|k| (b >> k) & 1));
        }
        let found = &mut self.found;
        sync.push_bits(&self.bits, |f| found.push((vc, f.bytes.to_vec())));
        let found = std::mem::take(&mut self.found);
        for (vc, f) in &found {
            self.frame(*vc, f, out);
        }
        self.found = found;
        self.found.clear();
    }

    fn frame(&mut self, vc: u8, f: &[u8], out: &mut Vec<Strip>) {
        self.stats.frames += 1;
        if vc == IR_VC {
            match msugs::decode_ir(f) {
                Some(d) if d.valid => self.ir_strips(&d, out),
                _ => self.stats.invalid += 1,
            }
            return;
        }
        let Some(ch) = VIS_VCS.iter().position(|&v| v == vc) else {
            return;
        };
        match msugs::vis_line(f) {
            Some((line, true)) => {
                if !line.is_multiple_of(msugs::VIS_REDUCE) {
                    return;
                }
                let row = line / msugs::VIS_REDUCE;
                let px: Vec<u8> = (0..msugs::VIS_WIDTH)
                    .map(|x| (msugs::vis_pixel(f, x * msugs::VIS_REDUCE as usize) >> 2) as u8)
                    .collect();
                self.stats.vis_frames += 1;
                self.stats.vis_lines = self.stats.vis_lines.max(row + 1);
                out.push(Strip {
                    picture: self.picture,
                    instrument: VIS_NAME,
                    ..Strip::new(ch as u16 + 1, row, 0, 1, msugs::VIS_WIDTH as u32, Arc::from(px))
                });
            }
            _ => self.stats.invalid += 1,
        }
    }

    fn ir_strips(&mut self, d: &IrFrame, out: &mut Vec<Strip>) {
        self.stats.ir_frames += 1;
        self.stats.ir_lines = self.stats.ir_lines.max(d.line + 1);
        let part = d.segment / 2;
        for c in 0..msugs::IR_CHANNELS {
            let px: Vec<u8> = d.pixels[c * msugs::IR_SEGMENT..(c + 1) * msugs::IR_SEGMENT]
                .iter()
                .map(|&v| (v >> 2) as u8)
                .collect();
            out.push(Strip {
                picture: self.picture,
                mask: 1 << part,
                instrument: IR_NAME,
                ..Strip::new(
                    c as u16 + 1,
                    d.line,
                    part * msugs::IR_SEGMENT as u32,
                    1,
                    msugs::IR_SEGMENT as u32,
                    Arc::from(px),
                )
            });
        }
    }
}

/// The report of [`MsuGsComponent`].
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MsuGsReport {
    /// The counters.
    pub stats: MsuGsStats,
}

impl Item for MsuGsReport {
    const KIND: &'static str = "satimg.msugs_report";
}

/// Good frames in; the CADUs as `satimg.cadu` items, the strips of both pictures and the report out.
#[derive(Default)]
pub struct MsuGsComponent {
    dec: MsuGsDecoder,
    strips: Vec<Strip>,
    pass: PassTimer,
}

impl Component for MsuGsComponent {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "satimg.msugs",
            vec![Port::typed::<FrameBytes>("in")],
            vec![
                item_port::<Strip>("out"),
                item_port::<Cadu>("cadus"),
                item_port::<MsuGsReport>("report"),
                pass_port(),
            ],
        )
        .with_settings(PassSetting::schema())
    }
    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        Ok(vec![
            item_format::<Strip>(),
            item_format::<Cadu>(),
            item_format::<MsuGsReport>(),
            pass_format(),
        ])
    }
    fn process(&mut self, io: &mut Io) {
        let mut cadus = Vec::new();
        for f in io.items::<FrameBytes>(0) {
            let b = &f.value.bytes;
            self.dec.push_cadu(b, &mut self.strips);
            if b.len() >= CADU_LEN {
                let mut c = Cadu {
                    bytes: [0; CADU_LEN],
                    corrected: None,
                    checked: false,
                    basis: Basis::Dual,
                    time: f.value.time_s,
                };
                c.bytes.copy_from_slice(&b[..CADU_LEN]);
                cadus.push(Timed {
                    time: f.time,
                    value: c,
                });
            }
        }
        if self.pass.after(io, !self.strips.is_empty(), 3) {
            self.dec.new_picture();
        }
        io.items_out::<Strip>(0)
            .extend(self.strips.drain(..).map(|s| Timed { time: 0.0, value: s }));
        io.items_out::<Cadu>(1).extend(cadus);
        if io.tick() {
            io.items_out::<MsuGsReport>(2).push(Timed {
                time: 0.0,
                value: MsuGsReport {
                    stats: self.dec.stats.clone(),
                },
            });
        }
    }
    instrument_owner!();
}

// ---------------------------------------------------------------------------------------------------------------------------------
// The carrier watch.

/// Seconds without a good frame after which a running demodulator searches the carrier again.
pub const RESEARCH_S: f64 = 1.0;

/// The reports of the gate and of the frame layer in; [`Control::RestartCarrier`] out when the gate has been open for
/// [`RESEARCH_S`] with no new good frame. It runs in the stage of the demodulator, so the feedback edge stays inside it.
#[derive(Default)]
pub struct CarrierWatch {
    last_good: u64,
    since_good_s: f64,
}

impl Component for CarrierWatch {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "satimg.rdas_watch",
            vec![Port::typed::<GateReport>("gate"), Port::typed::<TableReport>("table")],
            vec![item_port::<Control>("control")],
        )
    }
    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        Ok(vec![item_format::<Control>()])
    }
    fn process(&mut self, io: &mut Io) {
        if !io.tick() {
            return;
        }
        let g = last::<GateReport>(io, 0);
        let open = g.is_some_and(|g| g.open);
        let good = last::<TableReport>(io, 1).map_or(0, |r| r.stats.good);
        if good > self.last_good {
            self.last_good = good;
            self.since_good_s = 0.0;
        } else if open {
            self.since_good_s += REPORT_S;
        } else {
            self.since_good_s = 0.0;
        }
        if open && self.since_good_s >= RESEARCH_S {
            self.since_good_s = 0.0;
            io.items_out::<Control>(0).push(Timed {
                time: g.map_or(0.0, |g| g.time_s),
                value: Control::RestartCarrier,
            });
        }
    }
    fn apply(&mut self, id: SettingId, _: &Value) -> Result<Applied, SettingError> {
        Err(SettingError::NoSuchId { id })
    }
    fn values(&self) -> Values {
        no_values()
    }
}

// ---------------------------------------------------------------------------------------------------------------------------------
// The display.

fn display_declaration() -> ResultsDecl {
    with_series(
        ResultsDecl::new()
            .with_fields(FrameTableFields::SHAPE.fields)
            .with_fields(RdasFields::SHAPE.fields)
            .with_table(&FRAMES)
            .with_table(&VIRTUAL_CHANNELS),
    )
}

const NOTE_TEXT: &str = "Experimental: not confirmed on a real signal. No public document gives this link. Not confirmed by \
    a public document: the symbol rate 30.72 Msym/s and roll-off 0.5, NRZ-M, the CADU without Reed-Solomon code, the virtual \
    channel rule of bytes 5 and 11, the payload from byte 24, the MSU-GS sync word, the infrared and visible frame layouts, \
    the channels of the virtual channels and the second visible sample (not used). The visible picture is shown reduced 4:1.";

/// The display of `rdas`.
pub struct RdasDisplay {
    name: &'static str,
    values: Values,
    published: Published,
    hist: Histories,
    row: u64,
    summary: String,
    hint: String,
    key: String,
    /// The newest counts of the gate: samples let go by, their share in percent and the last line height.
    gate: Option<(i64, f64, f64)>,
}

impl RdasDisplay {
    /// A display for the satellite `name`.
    pub fn new(name: &'static str) -> Self {
        RdasDisplay {
            name,
            values: Values::defaults(&RdasSetting::schema()),
            published: published(display_declaration()),
            hist: Histories::default(),
            row: 0,
            summary: String::new(),
            hint: String::new(),
            key: String::new(),
            gate: None,
        }
    }
}

impl Component for RdasDisplay {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::sink(
            "satimg.rdas_display",
            vec![
                Port::typed::<FrontReport>("front"),
                Port::typed::<DemodReport>("demod"),
                Port::typed::<GateReport>("gate"),
                Port::typed::<TableReport>("table"),
                Port::typed::<MsuGsReport>("msugs"),
            ],
        )
        .with_settings(RdasSetting::schema())
    }
    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        Ok(Vec::new())
    }
    fn process(&mut self, io: &mut Io) {
        if !io.tick() {
            return;
        }
        let f = last::<FrontReport>(io, 0);
        let d = last::<DemodReport>(io, 1);
        let g = last::<GateReport>(io, 2);
        let none = TableReport::default();
        let r = last::<TableReport>(io, 3).unwrap_or(&none);
        let m = last::<MsuGsReport>(io, 4).cloned().unwrap_or_default();
        let t = f.map_or(0.0, |f| f.time_s);
        let low = f.is_some_and(|f| f.rate_too_low);
        let open = g.is_some_and(|g| g.open);
        let carrier = open && d.is_some_and(|d| d.state.tracking);
        let state = if low {
            0
        } else if r.stats.good > 0 && r.since_good_s < 0.5 && r.locked {
            3
        } else if r.locked {
            2
        } else if carrier {
            1
        } else {
            0
        };
        log_lines(self.published.results_mut(), &r.log);
        self.summary.clear();
        let name = self.name;
        // A write to a String cannot fail.
        let _ = match state {
            3 => write!(
                self.summary,
                "{name}, {} frames, {} MSU-GS frames, {} lines",
                r.stats.good, m.stats.frames, m.stats.ir_lines
            ),
            2 => write!(self.summary, "{name}, frame sync, frames failing"),
            1 => write!(self.summary, "{name}, symbols, no frame sync"),
            _ => write!(self.summary, "{name}, searching"),
        };
        self.hint.clear();
        if low {
            // A write to a String cannot fail.
            let _ = write!(
                self.hint,
                "The sample rate ({:.2} Msps) is too low: this link needs {:.0} Msps or more (a recording works).",
                f.map_or(0.0, |f| f.input_rate) / 1e6,
                MIN_RATE / 1e6
            );
        }
        let p = &mut self.published;
        status(p.results_mut(), state, f, d.filter(|_| open), &self.summary);
        if let Some(g) = g {
            self.gate = Some((
                g.skipped as i64,
                100.0 * g.skipped as f64 / g.samples.max(1) as f64,
                g.last_ratio,
            ));
        }
        p.put(&FrameTableFields {
            snr_db: d.filter(|_| open).map(|d| f64::from(d.state.es_n0_db)),
            symbol_rate_hz: d.map(|d| Hertz(d.state.symbol_rate)),
            bytes_corrected: 0,
            vcid: m.stats.last_vc.map(i64::from),
            stream: None,
            ..frame_table_fields(t, f, d, r, NOTE_TEXT)
        });
        p.put(&RdasFields {
            carrier_locked: carrier && d.is_some_and(|d| d.state.locked) || state == 3,
            packets: m.stats.frames as i64,
            lines_ir: i64::from(m.stats.ir_lines),
            lines_visible: i64::from(m.stats.vis_lines),
            gate_skipped: self.gate.map(|g| g.0),
            gate_skipped_pct: self.gate.map(|g| g.1),
            gate_line: self.gate.map(|g| g.2),
            hint: &self.hint,
        });
        let mut rr = r.clone();
        rr.stats.per_vc.clone_from(&m.stats.per_vc);
        publish_tables(p, &rr, &mut self.row, &mut self.key);
        self.hist.publish(self.published.results_mut(), d, None);
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        RdasSetting::from_id(id).ok_or(SettingError::NoSuchId { id })?;
        self.values.set(id, v)?;
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        self.values.clone()
    }
    fn results(&self) -> Option<&Results> {
        Some(self.published.results())
    }
}

// ---------------------------------------------------------------------------------------------------------------------------------
// The graph and the descriptor.

/// The picture layouts: the infrared picture (18 parts a line) and the reduced visible picture.
pub fn layouts() -> Vec<Layout> {
    let views = || {
        vec![
            NamedView::new("auto", "Automatic", View::FirstChannel),
            NamedView::new(
                "rgb123",
                "Channels 1, 2, 3",
                View::Rgb([(1, false), (2, false), (3, false)]),
            ),
        ]
    };
    vec![
        Layout {
            name: IR_NAME,
            width: msugs::IR_WIDTH as u32,
            max_rows: msugs::IR_LINES,
            full_mask: (1 << msugs::IR_SEGMENTS) - 1,
            views: views(),
        },
        Layout {
            name: VIS_NAME,
            width: msugs::VIS_WIDTH as u32,
            max_rows: msugs::VIS_LINES / msugs::VIS_REDUCE,
            full_mask: 1,
            views: views(),
        },
    ]
}

fn satellite(submode: Option<&str>) -> &'static str {
    match submode {
        Some(ARKTIKA_M) => "Arktika-M",
        _ => "Elektro-L",
    }
}

/// The recorded cost of each stage.
const COSTS: &str = include_str!("../../cost.txt");

/// The graph for the link of `v` and the satellite of `submode`, in three stages: the front end; the demodulator with
/// the frame layer and the carrier watch that steers it; the MSU-GS frames, the pictures and the display.
///
/// # Errors
///
/// [`GraphError::Schema`] if the picture layouts are refused.
pub fn builder(v: &Values, submode: Option<&str>) -> Result<GraphBuilder, GraphError> {
    let l = link(v);
    let pictures = PassPictures::new(layouts(), satellite(submode)).map_err(GraphError::Schema)?;
    Ok(GraphBuilder::new()
        .add("frontend", FrontEndComponent::new(l.clone(), MIN_RATE, false))
        .add("demod", GatedDemodComponent::new(l.clone(), GATE))
        .add("frames", FrameTableComponent::new(table(l.symbol_rate)).on_helper())
        .add("watch", CarrierWatch::default())
        .add("msugs", MsuGsComponent::default())
        .add("pictures", pictures)
        .add("display", RdasDisplay::new(satellite(submode)))
        .input("frontend.in")
        .connect("frontend.out", "demod.in")
        .connect("demod.soft", "frames.in")
        .connect("frames.frames", "msugs.in")
        .connect("msugs.out", "pictures.in")
        .connect("msugs.pass", "pictures.pass")
        .connect("frontend.report", "display.front")
        .connect("demod.report", "display.demod")
        .connect("demod.gate", "display.gate")
        .connect("frames.report", "display.table")
        .connect("msugs.report", "display.msugs")
        .connect("demod.gate", "watch.gate")
        .connect("frames.report", "watch.table")
        .feedback("watch.control", "demod.control")
        .stage("frontend", "demod", Stage::new("demod"))
        .stage("frames", "msugs", Stage::new("picture"))
        .costs(&CostTable::parse(COSTS), ID, "-")
        .output("pictures.picture")
        .output("msugs.cadus")
        .status_from("display")
        .clock(REPORT_S))
}

/// The receiver with the settings and the submode of `cfg`.
///
/// # Errors
///
/// The errors of [`GraphBuilder::build`] and of [`sdrkit_graph::StackGraphBuilder::build`], among them a refused setting.
pub fn receiver(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    let mut g = builder(&cfg.settings, cfg.submode.as_deref())?
        .catalog(&Catalog::of(cfg)?)
        .build()?;
    g.set_max_items(1 << 20);
    StackGraph::of(crate::typed::shape(ID), g)
        .min_rate(MIN_RATE, "this link needs 40 Msps or more (a recording works)")
        .command("clear", sdrkit_graph::CommandTarget::every("clear"))
        .route("symbol_rate", &["display.symbol_rate"])?
        .route("roll_off", &["display.roll_off"])?
        .route("pass_gap", &["msugs.pass_gap"])?
        .with_settings(&cfg.settings)
        .build()
}

fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    super::stack_receiver(receiver(cfg))
}

fn make_generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(Box::new(
        RdasGenerator::new(cfg.rate_hz, &cfg.settings).map_err(super::gen_error)?,
    ))
}

fn schema() -> Arc<sdrkit_core::Schema> {
    RdasSetting::schema()
}

/// Elektro-L and Arktika-M raw downlink.
pub static DESCRIPTOR: StackDescriptor = StackDescriptor {
    id: ID,
    name: "Elektro-L / Arktika-M RDAS",
    group: crate::GROUP,
    blurb: "The raw MSU-GS downlink of Elektro-L and Arktika-M on X band (BPSK, 30.72 Msym/s): infrared and visible pictures. It needs 40 Msps or more and an X-band dish with a down-converter; a recording is the usual input.",
    // The stack holds no frequency: the host sets it from the downlinks of data/links/xband_downlinks.csv (the
    // down-converted frequency with a down-converter). Without one the receiver follows the centre of the input.
    tuning: Tuning {
        min_hz: 950e6,
        max_hz: 9000e6,
        ..Tuning::anywhere(0.0, ASKED_RATE, MIN_RATE, 45e6)
    },
    submodes: &[
        SubMode {
            id: ELEKTRO_L,
            name: "Elektro-L",
            default_hz: 0.0,
        },
        SubMode {
            id: ARKTIKA_M,
            name: "Arktika-M",
            default_hz: 0.0,
        },
    ],
    settings: schema,
    make_receiver,
    make_generator: Some(make_generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[],
    auto_gain: AutoGain::Radio,
    verify: Some(&crate::verify::RDAS_HOOKS),
    benchmarks: None,
};

// ---------------------------------------------------------------------------------------------------------------------------------
// The test signal.

/// The MSU-GS streams of the test signal and the CADUs that carry them.
#[derive(Default)]
pub struct TestCadus {
    count: u32,
    ir: Vec<u8>,
    ir_at: usize,
    ir_line: u32,
    ir_segment: u32,
    vis: Vec<u8>,
    vis_at: usize,
    vis_line: u32,
}

/// The infrared test frame of `line` and `segment`.
pub fn test_ir_frame(line: u32, segment: u32) -> Vec<u8> {
    let x0 = (segment / 2) as usize * msugs::IR_SEGMENT;
    let px: Vec<u16> = (0..msugs::IR_CHANNELS * msugs::IR_SEGMENT)
        .map(|k| msugs::test_value(k / msugs::IR_SEGMENT, x0 + k % msugs::IR_SEGMENT, line))
        .collect();
    msugs::build_ir(line, segment, &px)
}

/// The visible test frame of picture line `y`: counter 4 y mod 17 200, channel 1 of the test picture.
pub fn test_vis_frame(y: u32) -> Vec<u8> {
    let px: Vec<u16> = (0..msugs::VIS_PIXELS).map(|x| msugs::test_value(0, x, y)).collect();
    msugs::build_vis((4 * y) % msugs::VIS_LINES, &px)
}

impl TestCadus {
    /// The next CADU as sent (randomised, sync word first): every third (count divisible by 3) on the visible channel 1
    /// (virtual channel 2), the others on the infrared channel (virtual channel 4).
    pub fn next_cadu(&mut self) -> Vec<u8> {
        let vis = self.count.is_multiple_of(3);
        let vc = if vis { VIS_VCS[0] } else { IR_VC };
        let mut c = vec![0u8; CADU_LEN];
        c[..4].copy_from_slice(&[0x1A, 0xCF, 0xFC, 0x1D]);
        c[4] = 0x40;
        c[5] = vc * 2;
        c[6..9].copy_from_slice(&self.count.to_be_bytes()[1..]);
        c[11] = vc * 2;
        for b in c[PAYLOAD_AT..].iter_mut() {
            *b = if vis { self.next_vis_byte() } else { self.next_ir_byte() };
        }
        sdrkit_proto_ccsds::randomiser::tm_bytes(&mut c[4..]);
        self.count = (self.count + 1) & 0xFF_FFFF;
        c
    }

    fn next_ir_byte(&mut self) -> u8 {
        if self.ir_at >= self.ir.len() {
            self.ir = test_ir_frame(self.ir_line, self.ir_segment);
            self.ir_at = 0;
            self.ir_segment += 2;
            if self.ir_segment >= 36 {
                self.ir_segment = 0;
                self.ir_line = (self.ir_line + 1) % msugs::IR_LINES;
            }
        }
        self.ir_at += 1;
        self.ir[self.ir_at - 1]
    }

    fn next_vis_byte(&mut self) -> u8 {
        if self.vis_at >= self.vis.len() {
            self.vis = test_vis_frame(self.vis_line);
            self.vis_at = 0;
            self.vis_line += 1;
        }
        self.vis_at += 1;
        self.vis[self.vis_at - 1]
    }
}

/// The channel bits of the test signal: the CADUs, NRZ-M coded.
#[derive(Default)]
pub struct RdasSource {
    cadus: TestCadus,
    level: u8,
}

impl BitSource for RdasSource {
    fn next_bits(&mut self, out: &mut Vec<u8>) {
        for b in self.cadus.next_cadu() {
            for k in (0..8).rev() {
                self.level ^= (b >> k) & 1;
                out.push(self.level);
            }
        }
    }
}

/// The test signal: BPSK of [`RdasSource`] with root raised cosine pulses, at unit power.
pub struct RdasGenerator {
    rate: f64,
    info: SignalInfo,
    m: Modulator<RdasSource>,
}

impl RdasGenerator {
    /// The signal at `rate` with the link of the settings `v`.
    ///
    /// # Errors
    ///
    /// [`GenError`] if the rate is below one sample a symbol or the pulse shaper refuses it.
    pub fn new(rate: f64, v: &Values) -> Result<Self, GenError> {
        let l = link(v);
        if rate < l.symbol_rate {
            return Err(GenError::RateTooLow {
                need: l.symbol_rate,
                got: rate,
            });
        }
        let p = ModParams::new(rate, l.symbol_rate, l.roll_off, Modulation::Bpsk);
        let info = p.signal_info();
        Ok(RdasGenerator {
            rate,
            info,
            m: Modulator::new(p, RdasSource::default())?,
        })
    }
}

impl Generator for RdasGenerator {
    fn rate_hz(&self) -> f64 {
        self.rate
    }
    fn generate(&mut self, out: &mut [sdrkit_core::Complex32]) -> usize {
        self.m.generate(out)
    }
    fn signal_info(&self) -> Option<SignalInfo> {
        Some(self.info)
    }
}
