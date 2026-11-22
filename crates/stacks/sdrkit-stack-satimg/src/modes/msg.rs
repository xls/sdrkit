//! MSG Raw Data (`msg_raw`): the raw SEVIRI downlink of Meteosat Second Generation, QPSK at 3.75 Msym/s with the CCSDS
//! concatenated code. The stack draws reduced pictures of the eleven standard channels and of the HRV channel, one disc
//! every 15 minutes.
//!
//! Standard: CCSDS 131.0-B-5 (code, randomiser, Reed-Solomon); CCSDS 732.0-B-4 (frame header, M_PDU); CCSDS 133.0-B-2
//! (packets).

use super::display::{instrument_choices, log_lines, published, put_link, status, view_choices, with_series, Histories};
use crate::components::instruments::instrument_owner;
use crate::components::link::{
    control_port, item_format, item_port, stage, CodeComponent, CodeReport, DemodComponent,
    DemodReport, FrameReport, FrameStats, FrontEndComponent, FrontReport, PacketReport,
    PacketStats,
};
use crate::components::{last, Control};
use crate::gen::{
    space_packet, test_value, BitSource, CcsdsCoder, CodeOptions, GenError, ModParams, Modulator,
    ZoneStream,
};
use crate::instruments::seviri::{
    test_user, SeviriDecoder, SeviriStats, APID, CHANNELS, HRV, HRV_ROWS, LINES, PARTS, ROWS,
    SEVIRI, VC, WIDTH,
};
use crate::psk::{LinkClass, LinkParams, Modulation, Search, SnrEstimator};
use crate::REPORT_S;
use sdrkit_codec_image::component::{Layout as PictureLayout, NamedView, Pictures};
use sdrkit_codec_image::strip::{Strip, View};
use crate::typed::{ImageRow, LinkFields, MsgRawFields, VirtualChannelRow, IMAGES, VIRTUAL_CHANNELS};
use sdrkit_core::interface::{Fields as _, Seconds};
use sdrkit_core::publish::{field, Published};
use sdrkit_core::settings::CatalogueSetting;
use sdrkit_core::{
    AutoGain, Complex32, ConfigError, GenConfig, Generator, LockProfile, Receiver, Results,
    ResultsDecl, Setting, SettingError, SettingId, SettingSet, SignalInfo, StackConfig,
    StackDescriptor, Tuning, Value, Values,
};
use sdrkit_graph::{
    no_values, Applied, Catalog, Component, ComponentInfo, Format, GraphBuilder, GraphError, Io,
    Item, Port, StackGraph, Timed,
};
use sdrkit_proto_ccsds::aos::{Layout as AosLayout, PrimaryHeader as AosHeader};
use sdrkit_proto_ccsds::channel::{ChannelCoder, ChannelConfig, ChannelDecoder};
use sdrkit_proto_ccsds::codesync::CodeConfig;
use sdrkit_proto_ccsds::framesync::{FrameSync, SyncConfig};
use sdrkit_proto_ccsds::packet::{Assembler, AssemblerConfig};
use std::fmt::Write as _;

/// The stack id.
pub const ID: &str = "msg_raw";
/// The least input rate.
pub const MIN_RATE: f64 = 5.5e6;
/// Symbols a second.
pub const SYMBOL_RATE: f64 = 3_750_000.0;
/// Bytes of a coded frame after the sync marker: five interleaved Reed-Solomon (255,223) code words.
pub const CODED_BYTES: usize = 1275;
/// Bytes of a channel access data unit: the sync marker and the coded frame.
pub const CADU_BYTES: usize = 4 + CODED_BYTES;
/// The transfer frame: 1115 bytes, a 6-byte header, the 2-byte M_PDU header and a packet zone of 1107 bytes.
pub const FRAME: AosLayout = AosLayout {
    frame_len: 1115,
    header_error_control: false,
    insert_zone: 0,
    ocf: false,
    fecf: false,
};
/// Seconds without a good frame after which the code is searched again.
const CODE_SEARCH_S: f64 = 6.0;

/// The link: QPSK with a roll-off of 0.35.
pub fn link() -> LinkParams {
    LinkParams {
        modulation: Modulation::Qpsk,
        symbol_rate: SYMBOL_RATE,
        roll_off: 0.35,
        class: LinkClass::Geo,
        search: Search::PowerLine {
            power: 4,
            min_ratio: 0.0,
        },
        snr: SnrEstimator::Decisions,
        bpsk_plus_is_zero: false,
        constellation: 256,
        interleaved: false,
    }
}

/// The rules of the code synchroniser: one sync marker in each 1279 bytes, no NRZ-M.
pub const CODE: CodeConfig = CodeConfig {
    frame_bits: 8 * CADU_BYTES as u32,
    nrzm: false,
    ..CodeConfig::GEO
};

const TEST: &str = "Test signal";

/// The picture shown.
pub const PICTURE: CatalogueSetting = CatalogueSetting {
    key: "picture",
    label: "Picture",
    help: "The eleven standard channels or the HRV channel.",
    choices: || instrument_choices(layouts()),
    default: SEVIRI,
    advanced: false,
    role: None,
};
/// The view of the picture.
pub const VIEW: CatalogueSetting = CatalogueSetting {
    key: "view",
    label: "View",
    help: "What the picture shows.",
    choices: || view_choices(layouts()),
    default: "rgb214",
    advanced: false,
    role: Some(sdrkit_core::SettingRole::PictureView),
};
/// Keep the newest lines in view.
pub const FOLLOW: Setting<field::Bool> = Setting::new("follow", "Follow", "", true);
/// The satellite number of the test signal.
pub const GEN_SCID: Setting<field::I64> =
    Setting::new("gen_scid", "Test satellite number", "", 323)
        .range(0.0, 1023.0, 1.0)
        .section(TEST)
        .advanced();
/// Lines of the test picture before it repeats.
pub const GEN_LINES: Setting<field::I64> = Setting::new("gen_lines", "Test picture lines", "", 400)
    .range(1.0, 4000.0, 1.0)
    .section(TEST)
    .advanced();
/// The first standard line of the test signal.
pub const GEN_START: Setting<field::I64> = Setting::new("gen_start", "Test first line", "", 0)
    .range(0.0, 4481.0, 1.0)
    .section(TEST)
    .advanced();

sdrkit_core::settings! {
    /// The settings of `msg_raw`, with the options of its test signal.
    pub enum MsgSetting {
        /// The picture shown.
        Picture = PICTURE,
        /// The view of the picture.
        View = VIEW,
        /// Keep the newest lines in view.
        Follow = FOLLOW,
        /// Test: the satellite number.
        GenScid = GEN_SCID,
        /// Test: picture lines.
        GenLines = GEN_LINES,
        /// Test: the first line.
        GenStart = GEN_START,
    }
}

/// The picture layouts: the reduced SEVIRI picture with eleven channels and the reduced HRV picture.
pub fn layouts() -> Vec<PictureLayout> {
    let mut views = vec![NamedView::new(
        "rgb214",
        "Colour (2, 1, 4)",
        View::Rgb([(2, false), (1, false), (4, true)]),
    )];
    for c in 1..=u16::from(CHANNELS) {
        views.push(NamedView::new(
            format!("ch{c}"),
            format!("Channel {c}"),
            View::Grey {
                channel: c,
                invert: false,
            },
        ));
    }
    vec![
        PictureLayout {
            name: SEVIRI,
            width: WIDTH as u32,
            max_rows: ROWS,
            full_mask: 1,
            views,
        },
        PictureLayout {
            name: HRV,
            width: WIDTH as u32,
            max_rows: HRV_ROWS,
            full_mask: 1,
            views: vec![NamedView::new(
                "hrv",
                "HRV",
                View::Grey {
                    channel: 1,
                    invert: false,
                },
            )],
        },
    ]
}

// ---------------------------------------------------------------------------------------------------------------------------------
// The frame, packet and instrument component.

/// The report of the SEVIRI decoder of [`MsgComponent`].
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SeviriReport {
    /// Counters.
    pub stats: SeviriStats,
    /// The satellite number of the last frame: its first ten bits.
    pub satellite: Option<u16>,
    /// New log lines.
    pub log: Vec<String>,
}
impl Item for SeviriReport {
    const KIND: &'static str = "satimg.seviri_report";
}

/// Decoded bits to frames to packets to SEVIRI strips: the frame synchroniser on 1279-byte units, the randomiser and
/// Reed-Solomon with interleave depth 5, the frame header, the packets of each virtual channel, and the SEVIRI packets.
pub struct MsgComponent {
    sync: Option<FrameSync>,
    channel: Option<ChannelDecoder>,
    asm: Assembler,
    buf: Vec<u8>,
    frames: FrameStats,
    packets: PacketStats,
    last_good: u64,
    last_failed: u64,
    corrected_now: u64,
    bits: u64,
    last_good_bit: u64,
    satellite: Option<u16>,
    dec: SeviriDecoder,
    strips: Vec<Strip>,
    log: Vec<String>,
}

impl Default for MsgComponent {
    fn default() -> Self {
        MsgComponent {
            sync: None,
            channel: None,
            asm: Assembler::new(AssemblerConfig::default()),
            buf: vec![0; CODED_BYTES],
            frames: FrameStats::default(),
            packets: PacketStats::default(),
            last_good: 0,
            last_failed: 0,
            corrected_now: 0,
            bits: 0,
            last_good_bit: 0,
            satellite: None,
            dec: SeviriDecoder::default(),
            strips: Vec::new(),
            log: Vec::new(),
        }
    }
}

impl MsgComponent {
    /// Takes one coded frame of 1275 bytes (the bytes after the sync marker).
    pub fn push_coded(&mut self, coded: &[u8]) {
        let Some(channel) = &mut self.channel else {
            return;
        };
        self.buf.copy_from_slice(&coded[..CODED_BYTES]);
        self.frames.checked += CODED_BYTES as u64;
        let ok = match channel.decode(&mut self.buf) {
            Ok(d) if d.ok() => {
                self.frames.corrected += u64::from(d.corrected);
                self.corrected_now += u64::from(d.corrected);
                true
            }
            _ => false,
        };
        if !ok {
            self.frames.failed += 1;
            return;
        }
        self.frames.good += 1;
        self.last_good_bit = self.bits;
        let Ok(f) = FRAME.parse(&self.buf) else {
            return;
        };
        let h = f.header;
        // The first ten bits of the frame are read as one number.
        let satellite = u16::from(h.version) << 8 | u16::from(h.spacecraft);
        self.satellite = Some(satellite);
        self.packets.last = Some((h.spacecraft, h.vcid, h.counter));
        *self.packets.per_vc.entry(h.vcid).or_insert(0) += 1;
        let (packets, dec, strips) = (&mut self.packets, &mut self.dec, &mut self.strips);
        self.asm
            .push_mpdu(h.vcid, Some(h.counter), f.first_header, f.zone, |p| {
                packets.packets += 1;
                *packets.per_apid.entry(p.header.apid).or_insert(0) += 1;
                if p.vcid == VC && p.header.apid == APID {
                    dec.push(satellite, p.header.count, p.data, strips);
                }
            });
        self.packets.lost = self.asm.stats().lost;
        if std::mem::take(&mut self.dec.disc_done) {
            self.log.push("a full disc is finished".into());
        }
    }
}

impl Component for MsgComponent {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "satimg.seviri",
            vec![Port::hard_bits("in"), control_port()],
            vec![
                item_port::<Strip>("out"),
                item_port::<FrameReport>("frames"),
                item_port::<PacketReport>("packets"),
                item_port::<SeviriReport>("report"),
            ],
        )
    }
    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.sync = Some(FrameSync::new(SyncConfig::cadu(CODED_BYTES)).map_err(stage)?);
        self.channel = Some(ChannelDecoder::new(ChannelConfig::rs_223(5)).map_err(stage)?);
        self.asm.reset();
        Ok(vec![
            item_format::<Strip>(),
            item_format::<FrameReport>(),
            item_format::<PacketReport>(),
            item_format::<SeviriReport>(),
        ])
    }
    fn process(&mut self, io: &mut Io) {
        let bits = io.hard_bits(0);
        if let Some(mut sync) = self.sync.take() {
            let mut coded = Vec::new();
            sync.push_bits(bits, |f| coded.extend_from_slice(f.bytes));
            self.bits += bits.len() as u64;
            for c in coded.as_chunks::<CODED_BYTES>().0 {
                self.push_coded(c);
            }
            let s = sync.stats();
            self.frames.syncs = s.frames;
            self.frames.locked = sync.locked();
            self.sync = Some(sync);
        }
        io.items_out::<Strip>(0)
            .extend(self.strips.drain(..).map(|s| Timed {
                time: 0.0,
                value: s,
            }));
        if io.tick() {
            let good_now = self.frames.good - self.last_good;
            let failed_now = self.frames.failed - self.last_failed;
            self.last_good = self.frames.good;
            self.last_failed = self.frames.failed;
            let checked = (good_now + failed_now) * CODED_BYTES as u64;
            let r = FrameReport {
                stats: self.frames,
                good_now,
                failed_now,
                byte_error_pct: if checked > 0 {
                    100.0 * std::mem::take(&mut self.corrected_now) as f64 / checked as f64
                } else {
                    0.0
                },
                since_good_s: (self.bits - self.last_good_bit) as f64 / SYMBOL_RATE,
                log: std::mem::take(&mut self.log),
                ..FrameReport::default()
            };
            io.items_out::<FrameReport>(1).push(Timed {
                time: 0.0,
                value: r,
            });
            io.items_out::<PacketReport>(2).push(Timed {
                time: 0.0,
                value: PacketReport {
                    stats: self.packets.clone(),
                },
            });
            io.items_out::<SeviriReport>(3).push(Timed {
                time: 0.0,
                value: SeviriReport {
                    stats: self.dec.stats,
                    satellite: self.satellite,
                    log: Vec::new(),
                },
            });
        }
    }
    instrument_owner!();
}

// ---------------------------------------------------------------------------------------------------------------------------------
// The display.

fn display_declaration() -> ResultsDecl {
    with_series(
        ResultsDecl::new()
            .with_fields(LinkFields::SHAPE.fields)
            .with_fields(MsgRawFields::SHAPE.fields)
            .with_table(&VIRTUAL_CHANNELS)
            .with_table(&IMAGES),
    )
}

/// The display: states, fields, tables, summary, histories and the new code search.
pub struct MsgDisplay {
    published: Published,
    hist: Histories,
    last_search_s: f64,
    first_row_s: Option<f64>,
    summary: String,
    key: String,
}

impl Default for MsgDisplay {
    fn default() -> Self {
        MsgDisplay {
            published: published(display_declaration()),
            hist: Histories::default(),
            last_search_s: 0.0,
            first_row_s: None,
            summary: String::new(),
            key: String::new(),
        }
    }
}

impl Component for MsgDisplay {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "satimg.msg_display",
            vec![
                Port::typed::<FrontReport>("front"),
                Port::typed::<DemodReport>("demod"),
                Port::typed::<CodeReport>("code"),
                Port::typed::<FrameReport>("frames"),
                Port::typed::<PacketReport>("packets"),
                Port::typed::<SeviriReport>("seviri"),
            ],
            vec![item_port::<Control>("control")],
        )
    }
    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.last_search_s = 0.0;
        Ok(vec![item_format::<Control>()])
    }
    fn process(&mut self, io: &mut Io) {
        if !io.tick() {
            return;
        }
        let f = last::<FrontReport>(io, 0);
        let d = last::<DemodReport>(io, 1);
        let c = last::<CodeReport>(io, 2);
        let fr = last::<FrameReport>(io, 3);
        let pk = last::<PacketReport>(io, 4);
        let none = SeviriReport::default();
        let m = last::<SeviriReport>(io, 5).unwrap_or(&none);
        let t = f.map_or(0.0, |f| f.time_s);
        if let Some(fr) = fr {
            log_lines(self.published.results_mut(), &fr.log);
        }
        let low = f.is_some_and(|f| f.rate_too_low);
        let code = c.is_some_and(|c| c.lock.locked);
        let good_recent = fr.is_some_and(|f| f.stats.good > 0 && f.since_good_s < 3.0);
        let state = if low {
            0
        } else if good_recent {
            3
        } else if code && fr.is_some_and(|f| f.stats.locked) {
            2
        } else if d.is_some_and(|d| d.state.locked && d.state.tracking) {
            1
        } else {
            0
        };
        // A code lock without a good frame for 6 s is searched again.
        if code
            && fr.is_some_and(|f| f.since_good_s >= CODE_SEARCH_S)
            && t - self.last_search_s >= CODE_SEARCH_S
        {
            self.last_search_s = t;
            io.items_out::<Control>(0).push(Timed {
                time: t,
                value: Control::CodeSearch,
            });
        }
        let s = &m.stats;
        let rows = s.rows + s.hrv_rows;
        if rows > 0 && self.first_row_s.is_none() {
            self.first_row_s = Some(t);
        }
        if rows == 0 {
            self.first_row_s = None;
        }
        self.summary.clear();
        // A write to a String cannot fail.
        let _ = match state {
            3 => write!(
                self.summary,
                "MSG raw data, {} lines, {} frames",
                s.lines,
                fr.map_or(0, |f| f.stats.good)
            ),
            2 => write!(self.summary, "MSG raw data, frame sync, frames failing"),
            1 => write!(self.summary, "MSG raw data, symbols, no frame sync"),
            _ if low => write!(
                self.summary,
                "this link needs {} Msps or more (a recording works)",
                MIN_RATE / 1e6
            ),
            _ => write!(self.summary, "MSG raw data, searching"),
        };
        let p = &mut self.published;
        put_link(p, t, f, d, c, fr, pk);
        status(p.results_mut(), state, f, d, &self.summary);
        p.put(&MsgRawFields {
            satellite_number: m.satellite.map(i64::from),
            lines: s.lines as i64,
            channel_rows: s.rows as i64,
            hrv_rows: s.hrv_rows as i64,
            seviri_packets: s.packets as i64,
            packets_refused: s.refused as i64,
            discs: s.discs as i64,
            note: "Experimental: not confirmed on a real signal. The satellite is shown by the number in the first ten bits of the \
                 frame. Not confirmed by a public document: QPSK at 3.75 Msym/s (only the rate of 3270 kbit/s is public), \
                 whether the frame header is an AOS or a TM header, virtual channel 0 and APID 2046, the time field, the clock \
                 numbers of each satellite, the line rule of 4482 lines in 900 s, the part of each packet and the parts stored \
                 in the opposite order, and the SEVIRI channel of each part. The pictures are reduced: every second standard \
                 line and pixel, every third HRV line and pixel. Not decoded: rapid scan, calibration, projection.",
        });
        for (name, n) in [(SEVIRI, s.rows), (HRV, s.hrv_rows)] {
            if n == 0 {
                continue;
            }
            IMAGES.put(
                p,
                name,
                &ImageRow {
                    product: name,
                    channel: None,
                    time: self.first_row_s.map(Seconds),
                    width: WIDTH as i64,
                    height: Some(i64::from(if name == SEVIRI { ROWS } else { HRV_ROWS })),
                },
            );
        }
        if let Some(pk) = pk {
            for (vc, frames) in &pk.stats.per_vc {
                self.key.clear();
                // A write to a String cannot fail.
                let _ = write!(self.key, "{vc}");
                VIRTUAL_CHANNELS.put(
                    p,
                    &self.key,
                    &VirtualChannelRow {
                        vcid: i64::from(*vc),
                        frames: *frames as i64,
                    },
                );
            }
        }
        self.hist.publish(self.published.results_mut(), d, fr);
    }
    fn apply(&mut self, id: SettingId, _: &Value) -> Result<Applied, SettingError> {
        Err(SettingError::NoSuchId { id })
    }
    fn values(&self) -> Values {
        no_values()
    }
    fn results(&self) -> Option<&Results> {
        Some(self.published.results())
    }
}

// ---------------------------------------------------------------------------------------------------------------------------------
// The graph and the descriptor.

/// The MSG graph.
///
/// # Errors
///
/// [`GraphError::Schema`] if the picture layouts produce no valid schema.
pub fn builder() -> Result<GraphBuilder, GraphError> {
    let pictures = Pictures::new(layouts()).map_err(GraphError::Schema)?;
    Ok(GraphBuilder::new()
        .add("frontend", FrontEndComponent::new(link(), MIN_RATE, false))
        .add("demod", DemodComponent::new(link()))
        .add("decoder", CodeComponent::new(CODE))
        .add("frames", MsgComponent::default())
        .add("pictures", pictures)
        .add("display", MsgDisplay::default())
        .input("frontend.in")
        .connect("frontend.out", "demod.in")
        .connect("demod.soft", "decoder.in")
        .connect("decoder.out", "frames.in")
        .connect("frames.out", "pictures.in")
        .connect("frontend.report", "display.front")
        .connect("demod.report", "display.demod")
        .connect("decoder.report", "display.code")
        .connect("frames.frames", "display.frames")
        .connect("frames.packets", "display.packets")
        .connect("frames.report", "display.seviri")
        .feedback("display.control", "decoder.control")
        .output("pictures.picture")
        .status_from("display")
        .clock(REPORT_S))
}

/// The MSG receiver: the graph with the stack's settings.
///
/// # Errors
///
/// The errors of [`GraphBuilder::build`] and of [`sdrkit_graph::StackGraphBuilder::build`], among them a refused setting.
pub fn receiver(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    let mut g = builder()?.catalog(&Catalog::of(cfg)?).build()?;
    g.set_max_items(1 << 20);
    StackGraph::of(crate::typed::shape(ID), g)
        .min_rate(MIN_RATE, "MSG raw data needs at least 5.5 MHz of samples")
        .route("picture", &["pictures.instrument"])?
        .route("view", &["pictures.view"])?
        .route("follow", &["pictures.follow"])?
        .command("clear", sdrkit_graph::CommandTarget::every("clear"))
        .with_settings(&cfg.settings)
        .build()
}

fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    super::stack_receiver(receiver(cfg))
}

fn make_generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(Box::new(
        MsgGenerator::from_config(cfg).map_err(super::gen_error)?,
    ))
}

/// The stack.
pub static DESCRIPTOR: StackDescriptor = StackDescriptor {
    id: ID,
    name: "MSG Raw Data",
    group: crate::GROUP,
    blurb: "The raw SEVIRI downlink of Meteosat Second Generation (QPSK, 3.75 Msym/s): reduced pictures of eleven channels and of HRV, a disc every 15 minutes. It needs a large fixed dish.",
    tuning: Tuning {
        min_hz: 1670e6,
        max_hz: 1710e6,
        // The default frequency is the one of the test signal. The host gives the frequency of a session.
        ..Tuning::anywhere(1686.83e6, 6e6, MIN_RATE, 5.25e6)
    },
    submodes: &[],
    settings: MsgSetting::schema,
    make_receiver,
    make_generator: Some(make_generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[],
    auto_gain: AutoGain::Radio,
    verify: None,
    benchmarks: None,
};

// ---------------------------------------------------------------------------------------------------------------------------------
// The generator.

/// The transfer frame of satellite number `satellite` (its first ten bits), virtual channel 0, with frame count `count`.
pub fn test_frame(
    satellite: u16,
    count: u32,
    first_header: sdrkit_proto_ccsds::tm::FirstHeader,
    zone: &[u8],
) -> Vec<u8> {
    let h = AosHeader {
        version: ((satellite >> 8) & 3) as u8,
        spacecraft: satellite as u8,
        vcid: VC,
        counter: count & 0xFF_FFFF,
        ..AosHeader::default()
    };
    let mut out = vec![0u8; FRAME.frame_len];
    let zl = FRAME.data_zone_len().min(zone.len());
    if FRAME
        .build(&h, &[], first_header, &zone[..zl], &mut out)
        .is_err()
    {
        out.fill(0);
    }
    out
}

/// The channel bits of the MSG test signal: steps of 16 SEVIRI packets for the standard line `L`, which rises by 3 a step.
pub struct MsgSource {
    coder: CcsdsCoder,
    channel: Option<ChannelCoder>,
    zones: ZoneStream,
    satellite: u16,
    lines: u32,
    line: u32,
    seq: u16,
    count: u32,
    cadu: Vec<u8>,
}

impl BitSource for MsgSource {
    fn next_bits(&mut self, out: &mut Vec<u8>) {
        let zl = FRAME.data_zone_len();
        while self.zones.len() < zl {
            for part in 0..PARTS {
                let user = test_user(self.satellite, part, self.line, self.lines, test_value);
                let seq = (self.seq & !15) | u16::from(part);
                self.zones.push(&space_packet(APID, 3, seq, &user, false));
            }
            self.seq = self.seq.wrapping_add(16) & 0x3FFF;
            self.line = (self.line + 3) % (LINES - LINES % 3);
        }
        let (fhp, zone) = self.zones.take(zl);
        let frame = test_frame(self.satellite, self.count, fhp, &zone);
        self.count = (self.count + 1) & 0xFF_FFFF;
        self.cadu.clear();
        let coded = match &mut self.channel {
            Some(c) => c.encode(&frame, &mut self.cadu).is_ok(),
            None => false,
        };
        if !coded {
            self.cadu.resize(CADU_BYTES, 0);
        }
        self.coder.push(&self.cadu, out);
    }
}

/// The MSG test signal: the clean signal at unit mean power.
pub struct MsgGenerator {
    rate: f64,
    info: SignalInfo,
    m: Modulator<MsgSource>,
}

impl MsgGenerator {
    /// From a configuration: `gen_scid`, `gen_lines` and `gen_start`. The schema of the settings must be [`MsgSetting`].
    ///
    /// # Errors
    ///
    /// [`GenError`] if the modulator refuses the rate.
    pub fn from_config(cfg: &GenConfig) -> Result<Self, GenError> {
        let l = link();
        let modem = ModParams::new(cfg.rate_hz, l.symbol_rate, l.roll_off, l.modulation);
        let v = &cfg.settings;
        let src = MsgSource {
            coder: CcsdsCoder::new(CodeOptions {
                nrzm: false,
                invert_g2: true,
                conv: true,
            }),
            channel: ChannelCoder::new(ChannelConfig::rs_223(5)).ok(),
            zones: ZoneStream::default(),
            satellite: v.get(&GEN_SCID).clamp(0, 1023) as u16,
            lines: v.get(&GEN_LINES).clamp(1, 4000) as u32,
            line: (v.get(&GEN_START).clamp(0, i64::from(LINES) - 1) as u32) / 3 * 3,
            seq: 0,
            count: 0,
            cadu: Vec::with_capacity(CADU_BYTES),
        };
        Ok(MsgGenerator {
            rate: cfg.rate_hz,
            info: modem.signal_info(),
            m: Modulator::new(modem, src)?,
        })
    }
}

impl Generator for MsgGenerator {
    fn rate_hz(&self) -> f64 {
        self.rate
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        self.m.generate(out)
    }
    fn signal_info(&self) -> Option<SignalInfo> {
        Some(self.info)
    }
}
