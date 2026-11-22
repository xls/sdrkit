//! AWS L-band (`aws`): the direct broadcast of the Arctic Weather Satellite at 1707 MHz, QPSK at 1.785 Msym/s with the
//! CCSDS concatenated code and TM transfer frames. The stack draws the scans of the microwave radiometer.
//!
//! Standard: Arctic Weather Satellite Direct Data Broadcast Interface Control Document AWS-OSE-ICD-0083; CCSDS 131.0-B-5;
//! CCSDS 132.0-B-3; CCSDS 133.0-B-2.

use super::display::{
    link_fields,log_lines, published, status, view_choices, with_series, Histories};
use crate::components::instruments::instrument_owner;
use crate::components::link::{
    control_port, item_format, item_port, stage, CodeComponent, CodeReport, DemodComponent,
    DemodReport, FrameReport, FrameStats, FrontEndComponent, FrontReport, PacketReport,
    PacketStats,
};
use crate::components::{last, Control};
use crate::gen::{
    test_value, BitSource, CcsdsCoder, CodeOptions, GenError, ModParams, Modulator, ZoneStream,
};
use crate::instruments::mwr::{
    field_header, test_record, MwrDecoder, MwrStats, CHANNELS, IDLE_VC, MAX_ROWS, SCIENCE_APID,
    SCIENCE_VC, SPACECRAFT as AWS_SPACECRAFT, SPOTS,
};
use crate::psk::{LinkClass, LinkParams, Modulation, Search, SnrEstimator};
use crate::REPORT_S;
use sdrkit_codec_image::component::{Layout as PictureLayout, NamedView, Pictures};
use sdrkit_codec_image::strip::{Strip, View};
use crate::typed::{
    AwsApidRow, AwsFields, AwsVirtualChannelRow, ImageRow, LinkFields, AWS_APIDS,
    AWS_VIRTUAL_CHANNELS, IMAGES,
};
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
use sdrkit_proto_ccsds::channel::{ChannelCoder, ChannelConfig, ChannelDecoder};
use sdrkit_proto_ccsds::codesync::CodeConfig;
use sdrkit_proto_ccsds::framesync::{FrameSync, SyncConfig};
use sdrkit_proto_ccsds::packet::{
    build, Assembler, AssemblerConfig, PrimaryHeader as PacketHeader, SeqFlags,
};
use sdrkit_proto_ccsds::tm::{Layout as TmLayout, PrimaryHeader as TmHeader};
use sdrkit_util_bits::crc::catalog::CRC16_IBM_3740;
use std::fmt::Write as _;

/// The stack id.
pub const ID: &str = "aws";
/// The least input rate.
pub const MIN_RATE: f64 = 4e6;
/// Symbols a second.
pub const SYMBOL_RATE: f64 = 1_785_000.0;
/// Bytes of a coded frame after the sync marker: five interleaved Reed-Solomon (255,223) code words.
pub const CODED_BYTES: usize = 1275;
/// Bytes of a channel access data unit: the sync marker and the coded frame.
pub const CADU_BYTES: usize = 4 + CODED_BYTES;
/// The transfer frame: 1115 bytes, a primary header of 6 bytes and a data field of 1109 bytes, no trailer.
pub const FRAME: TmLayout = TmLayout {
    frame_len: 1115,
    secondary_header_len: 0,
    ocf: false,
    fecf: false,
};
/// Source bytes of a science packet of the test signal.
const GEN_SEGMENT: usize = 1000;
/// Seconds without a good frame after which the code is searched again.
const CODE_SEARCH_S: f64 = 6.0;

/// The packet error control of a packet with primary header `header` and the data field `data` before the control field:
/// CRC-16-CCITT over the whole packet.
pub fn packet_crc(header: &[u8; 6], data: &[u8]) -> u16 {
    let mut d = CRC16_IBM_3740.digest();
    d.update(header);
    d.update(data);
    d.finalize()
}

/// The link: QPSK with a roll-off of 0.45.
pub fn link() -> LinkParams {
    LinkParams {
        modulation: Modulation::Qpsk,
        symbol_rate: SYMBOL_RATE,
        roll_off: 0.45,
        class: LinkClass::Leo,
        search: Search::Template { range_hz: 75e3 },
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
    ..CodeConfig::LEO
};

const TEST: &str = "Test signal";

/// The view of the picture.
pub const VIEW: CatalogueSetting = CatalogueSetting {
    key: "view",
    label: "View",
    help: "The radiometer channel shown.",
    choices: || view_choices(vec![picture_layout()]),
    default: "ch1",
    advanced: false,
    role: Some(sdrkit_core::SettingRole::PictureView),
};
/// Keep the newest scans in view.
pub const FOLLOW: Setting<field::Bool> = Setting::new("follow", "Follow", "", true);
/// Scans of the test picture before it repeats.
pub const GEN_LINES: Setting<field::I64> = Setting::new("gen_lines", "Test picture scans", "", 400)
    .range(1.0, 4000.0, 1.0)
    .section(TEST)
    .advanced();

sdrkit_core::settings! {
    /// The settings of `aws`, with the options of its test signal.
    pub enum AwsSetting {
        /// The view of the picture.
        View = VIEW,
        /// Keep the newest scans in view.
        Follow = FOLLOW,
        /// Test: picture scans.
        GenLines = GEN_LINES,
    }
}

/// The receiver of a channel: 54, 89, 183 or 325 GHz.
fn receiver_ghz(channel: u16) -> u16 {
    match channel {
        1..=8 => 54,
        9 => 89,
        10..=15 => 183,
        _ => 325,
    }
}

/// The picture layout of the radiometer: 145 scene spots a scan, 19 channels, one grey view for each channel.
pub fn picture_layout() -> PictureLayout {
    let views = (1..=CHANNELS as u16)
        .map(|c| {
            NamedView::new(
                format!("ch{c}"),
                format!("Channel {c} ({} GHz receiver)", receiver_ghz(c)),
                View::Grey {
                    channel: c,
                    invert: false,
                },
            )
        })
        .collect();
    PictureLayout {
        name: "MWR",
        width: SPOTS as u32,
        max_rows: MAX_ROWS,
        full_mask: 1,
        views,
    }
}

// ---------------------------------------------------------------------------------------------------------------------------------
// The frame, packet and instrument component.

/// The report of the radiometer decoder of [`AwsComponent`].
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MwrReport {
    /// Counters.
    pub stats: MwrStats,
    /// The spacecraft id of the last frame, 10 bits.
    pub spacecraft: Option<u16>,
    /// Idle frames (virtual channel 7).
    pub idle_frames: u64,
}
impl Item for MwrReport {
    const KIND: &'static str = "satimg.mwr_report";
}

/// Decoded bits to frames to packets to radiometer strips: the frame synchroniser on 1279-byte units, the randomiser and
/// Reed-Solomon with interleave depth 5, the TM transfer frame, the packets of each virtual channel, and the science packets
/// of virtual channel 3.
pub struct AwsComponent {
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
    spacecraft: Option<u16>,
    idle_frames: u64,
    dec: MwrDecoder,
    strips: Vec<Strip>,
}

impl Default for AwsComponent {
    fn default() -> Self {
        AwsComponent {
            sync: None,
            channel: None,
            asm: Assembler::new(AssemblerConfig {
                count_bits: 8,
                ..AssemblerConfig::default()
            }),
            buf: vec![0; CODED_BYTES],
            frames: FrameStats::default(),
            packets: PacketStats::default(),
            last_good: 0,
            last_failed: 0,
            corrected_now: 0,
            bits: 0,
            last_good_bit: 0,
            spacecraft: None,
            idle_frames: 0,
            dec: MwrDecoder::default(),
            strips: Vec::new(),
        }
    }
}

impl AwsComponent {
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
        if h.version != 0 {
            self.packets.refused += 1;
            return;
        }
        self.spacecraft = Some(h.spacecraft);
        self.packets.last = Some((h.spacecraft as u8, h.vcid, u32::from(h.vc_count)));
        if h.vcid == IDLE_VC {
            self.idle_frames += 1;
            self.packets.fill_frames += 1;
            return;
        }
        *self.packets.per_vc.entry(h.vcid).or_insert(0) += 1;
        let (packets, dec, strips) = (&mut self.packets, &mut self.dec, &mut self.strips);
        self.asm.push_mpdu(
            h.vcid,
            Some(u32::from(h.vc_count)),
            h.first_header,
            f.data,
            |p| {
                packets.packets += 1;
                *packets.per_apid.entry(p.header.apid).or_insert(0) += 1;
                if p.vcid == SCIENCE_VC && p.header.apid == SCIENCE_APID && p.data.len() >= 2 {
                    if p.gap_before {
                        dec.restart();
                    }
                    let (data, check) = p.data.split_at(p.data.len() - 2);
                    let ok = p
                        .header
                        .bytes()
                        .is_ok_and(|h| packet_crc(&h, data).to_be_bytes() == check);
                    packets.crc_errors += u64::from(!ok);
                    dec.push(p.header.flags.bits(), data, ok, strips);
                }
            },
        );
        self.packets.lost = self.asm.stats().lost;
    }
}

impl Component for AwsComponent {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "satimg.aws_frames",
            vec![Port::hard_bits("in"), control_port()],
            vec![
                item_port::<Strip>("out"),
                item_port::<FrameReport>("frames"),
                item_port::<PacketReport>("packets"),
                item_port::<MwrReport>("report"),
            ],
        )
    }
    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.sync = Some(FrameSync::new(SyncConfig::cadu(CODED_BYTES)).map_err(stage)?);
        self.channel = Some(ChannelDecoder::new(ChannelConfig::rs_223(5)).map_err(stage)?);
        self.asm.reset();
        self.dec.restart();
        Ok(vec![
            item_format::<Strip>(),
            item_format::<FrameReport>(),
            item_format::<PacketReport>(),
            item_format::<MwrReport>(),
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
            io.items_out::<MwrReport>(3).push(Timed {
                time: 0.0,
                value: MwrReport {
                    stats: self.dec.stats,
                    spacecraft: self.spacecraft,
                    idle_frames: self.idle_frames,
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
            .with_fields(AwsFields::SHAPE.fields)
            .with_table(&AWS_VIRTUAL_CHANNELS)
            .with_table(&AWS_APIDS)
            .with_table(&IMAGES),
    )
}

/// The display: states, fields, tables, summary, histories and the new code search.
pub struct AwsDisplay {
    published: Published,
    hist: Histories,
    last_search_s: f64,
    first_row_s: Option<f64>,
    summary: String,
    key: String,
}

impl Default for AwsDisplay {
    fn default() -> Self {
        AwsDisplay {
            published: published(display_declaration()),
            hist: Histories::default(),
            last_search_s: 0.0,
            first_row_s: None,
            summary: String::new(),
            key: String::new(),
        }
    }
}

impl Component for AwsDisplay {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "satimg.aws_display",
            vec![
                Port::typed::<FrontReport>("front"),
                Port::typed::<DemodReport>("demod"),
                Port::typed::<CodeReport>("code"),
                Port::typed::<FrameReport>("frames"),
                Port::typed::<PacketReport>("packets"),
                Port::typed::<MwrReport>("mwr"),
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
        let none = MwrReport::default();
        let m = last::<MwrReport>(io, 5).unwrap_or(&none);
        let t = f.map_or(0.0, |f| f.time_s);
        if let Some(fr) = fr {
            log_lines(self.published.results_mut(), &fr.log);
        }
        let code = c.is_some_and(|c| c.lock.locked);
        let good_recent = fr.is_some_and(|f| f.stats.good > 0 && f.since_good_s < 3.0);
        let state = if good_recent {
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
        if m.stats.rows > 0 && self.first_row_s.is_none() {
            self.first_row_s = Some(t);
        }
        if m.stats.rows == 0 {
            self.first_row_s = None;
        }
        self.summary.clear();
        // A write to a String cannot fail.
        let _ = match state {
            3 => write!(
                self.summary,
                "AWS L-band, {} scans, {} frames",
                m.stats.rows,
                fr.map_or(0, |f| f.stats.good)
            ),
            2 => write!(self.summary, "AWS L-band, frame sync, frames failing"),
            1 => write!(self.summary, "AWS L-band, symbols, no frame sync"),
            _ => write!(self.summary, "AWS L-band, searching"),
        };
        let p = &mut self.published;
        p.put(&LinkFields {
            spacecraft: m.spacecraft.map(i64::from),
            ..link_fields(t, f, d, c, fr, pk)
        });
        status(p.results_mut(), state, f, d, &self.summary);
        p.put(&AwsFields {
            lines: i64::from(m.stats.rows),
            science_packets: m.stats.packets as i64,
            packet_crc_errors: m.stats.crc_errors as i64,
            odd_records: m.stats.odd_records as i64,
            idle_frames: m.idle_frames as i64,
            note: "Experimental: not confirmed on a real signal. The scan record is taken from right after the 15-byte data field \
                 header of each science packet, as the interface document describes the packet. Not decoded: the navigation and \
                 attitude packets (APID 51), the time packet, the housekeeping and calibration parts of the scan record, and \
                 the stored-data virtual channels 1 and 2.",
        });
        if m.stats.rows > 0 {
            IMAGES.put(
                p,
                "MWR",
                &ImageRow {
                    product: "MWR",
                    channel: None,
                    time: self.first_row_s.map(Seconds),
                    width: SPOTS as i64,
                    height: Some(i64::from(m.stats.rows)),
                },
            );
        }
        if let Some(pk) = pk {
            for (vc, frames) in &pk.stats.per_vc {
                self.key.clear();
                // A write to a String cannot fail.
                let _ = write!(self.key, "{vc}");
                AWS_VIRTUAL_CHANNELS.put(
                    p,
                    &self.key,
                    &AwsVirtualChannelRow {
                        vcid: i64::from(*vc),
                        frames: *frames as i64,
                    },
                );
            }
            for (apid, packets) in &pk.stats.per_apid {
                self.key.clear();
                // A write to a String cannot fail.
                let _ = write!(self.key, "{apid}");
                AWS_APIDS.put(
                    p,
                    &self.key,
                    &AwsApidRow {
                        apid: i64::from(*apid),
                        packets: *packets as i64,
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

/// The AWS graph.
///
/// # Errors
///
/// [`GraphError::Schema`] if the picture layout produces no valid schema.
pub fn builder() -> Result<GraphBuilder, GraphError> {
    let pictures = Pictures::new(vec![picture_layout()]).map_err(GraphError::Schema)?;
    Ok(GraphBuilder::new()
        .add("frontend", FrontEndComponent::new(link(), MIN_RATE, false))
        .add("demod", DemodComponent::new(link()))
        .add("decoder", CodeComponent::new(CODE))
        .add("frames", AwsComponent::default())
        .add("pictures", pictures)
        .add("display", AwsDisplay::default())
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
        .connect("frames.report", "display.mwr")
        .feedback("display.control", "decoder.control")
        .output("pictures.picture")
        .status_from("display")
        .clock(REPORT_S))
}

/// The AWS receiver: the graph with the stack's settings.
///
/// # Errors
///
/// The errors of [`GraphBuilder::build`] and of [`sdrkit_graph::StackGraphBuilder::build`], among them a refused setting.
pub fn receiver(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    let mut g = builder()?.catalog(&Catalog::of(cfg)?).build()?;
    g.set_max_items(1 << 20);
    StackGraph::of(crate::typed::shape(ID), g)
        .min_rate(MIN_RATE, "AWS L-band needs at least 4 MHz of samples")
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
        AwsGenerator::from_config(cfg).map_err(super::gen_error)?,
    ))
}

/// The stack.
pub static DESCRIPTOR: StackDescriptor = StackDescriptor {
    id: ID,
    name: "AWS L-band",
    group: crate::GROUP,
    blurb: "The direct broadcast of the Arctic Weather Satellite (QPSK, 1.785 Msym/s): scans of the microwave radiometer in 19 channels. It needs a tracking dish.",
    tuning: Tuning {
        min_hz: 1690e6,
        max_hz: 1710e6,
        // The default frequency is the one of the test signal. The host gives the frequency of a session.
        ..Tuning::anywhere(1707e6, 6e6, MIN_RATE, 2.5e6)
    },
    submodes: &[],
    settings: AwsSetting::schema,
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

/// The science packets of the scan record of `line`: source data of 1000 bytes (the last 599), each behind the data field
/// header, with the packet error control. `seq` is the sequence count of the first packet.
pub fn test_packets(line: u32, seq: u16) -> Vec<Vec<u8>> {
    let record = test_record(line, test_value);
    let pieces: Vec<&[u8]> = record.chunks(GEN_SEGMENT).collect();
    pieces
        .iter()
        .enumerate()
        .map(|(i, piece)| {
            let flags = if i == 0 {
                SeqFlags::First
            } else if i + 1 == pieces.len() {
                SeqFlags::Last
            } else {
                SeqFlags::Continuation
            };
            let mut h =
                PacketHeader::telemetry(SCIENCE_APID, flags, seq.wrapping_add(i as u16) & 0x3FFF);
            h.secondary_header = true;
            let mut data = field_header(line).to_vec();
            data.extend_from_slice(piece);
            data.extend_from_slice(&[0, 0]);
            let mut out = Vec::with_capacity(data.len() + 6);
            if build(&h, &data, &mut out).is_err() {
                out.clear();
                return out;
            }
            let n = out.len() - 2;
            let check = CRC16_IBM_3740.checksum(&out[..n]);
            out[n..].copy_from_slice(&check.to_be_bytes());
            out
        })
        .collect()
}

/// The transfer frame with virtual channel frame count `count` for a data field and its first header pointer.
pub fn test_frame(
    count: u8,
    first_header: sdrkit_proto_ccsds::tm::FirstHeader,
    data: &[u8],
) -> Vec<u8> {
    let h = TmHeader {
        version: 0,
        spacecraft: AWS_SPACECRAFT,
        vcid: SCIENCE_VC,
        mc_count: count,
        vc_count: count,
        segment_length_id: 3,
        first_header,
        ..TmHeader::default()
    };
    let mut out = vec![0u8; FRAME.frame_len];
    if FRAME.build(&h, &[], data, None, &mut out).is_err() {
        out.fill(0);
    }
    out
}

/// The channel bits of the AWS test signal.
pub struct AwsSource {
    coder: CcsdsCoder,
    channel: Option<ChannelCoder>,
    zones: ZoneStream,
    count: u8,
    seq: u16,
    line: u32,
    lines: u32,
    cadu: Vec<u8>,
}

impl BitSource for AwsSource {
    fn next_bits(&mut self, out: &mut Vec<u8>) {
        let zone = FRAME.data_len();
        while self.zones.len() < zone {
            let packets = test_packets(self.line % self.lines, self.seq);
            self.line += 1;
            self.seq = self.seq.wrapping_add(packets.len() as u16) & 0x3FFF;
            for p in &packets {
                self.zones.push(p);
            }
        }
        let (first_header, data) = self.zones.take(zone);
        let frame = test_frame(self.count, first_header, &data);
        self.count = self.count.wrapping_add(1);
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

/// The AWS test signal: the clean signal at unit mean power.
pub struct AwsGenerator {
    rate: f64,
    info: SignalInfo,
    m: Modulator<AwsSource>,
}

impl AwsGenerator {
    /// From a configuration: `gen_lines`. The schema of the settings must be [`AwsSetting`].
    ///
    /// # Errors
    ///
    /// [`GenError`] if the modulator refuses the rate.
    pub fn from_config(cfg: &GenConfig) -> Result<Self, GenError> {
        let l = link();
        let modem = ModParams::new(cfg.rate_hz, l.symbol_rate, l.roll_off, l.modulation);
        let src = AwsSource {
            coder: CcsdsCoder::new(CodeOptions {
                nrzm: false,
                invert_g2: true,
                conv: true,
            }),
            channel: ChannelCoder::new(ChannelConfig::rs_223(5)).ok(),
            zones: ZoneStream::default(),
            count: 0,
            seq: 0,
            line: 0,
            lines: cfg.settings.get(&GEN_LINES).clamp(1, 4000) as u32,
            cadu: Vec::with_capacity(CADU_BYTES),
        };
        Ok(AwsGenerator {
            rate: cfg.rate_hz,
            info: modem.signal_info(),
            m: Modulator::new(modem, src)?,
        })
    }
}

impl Generator for AwsGenerator {
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
