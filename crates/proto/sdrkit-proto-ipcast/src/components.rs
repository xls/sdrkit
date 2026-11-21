//! The graph components over the plain types of this crate: `ipcast.bbframe`, `ipcast.alp`, `ipcast.ip` and `ipcast.route`, and
//! the [`PlpPair`] item that names the data PLP and common PLP whose transport streams `ipcast.bbframe` merges.
//! Output packets come from a [`PacketPool`], so the components do not allocate per packet once the pool has grown.

use crate::adapt::UpDeframer;
use crate::alp::{AlpEvent, AlpReader, PacketType};
use crate::bbheader::{BbHeader, StreamFormat, System, BBHEADER_LEN};
use crate::flow::FlowTable;
use crate::gse::{GseReader, Label};
use crate::ip::Reassembler;
use crate::merge::CommonMerge;
use crate::pcap::{LinkType, PcapWriter};
use crate::route::RouteAssembler;
use sdrkit_core::interface::{Family, Fields, StackShape};
use sdrkit_core::publish::{field, keys, Published};
use sdrkit_core::{
    Choice, ConfigError, Results, Setting, SettingError, SettingId, SettingSet, Value, Values,
};
use sdrkit_graph::TagKind;
use sdrkit_graph::{
    no_values, values_of, Applied, Component, ComponentInfo, Format, Io, Item, Kept, Kind, Packet,
    PacketPool, Port, RateFrom, Registry, Timed,
};
use sdrkit_proto_mpegts::stream::{self as ts_stream, TsOut, TsPacket};
use std::net::IpAddr;
use std::sync::OnceLock;

/// Registers every component of this crate.
pub fn register(r: &mut Registry) {
    r.add("ipcast.bbframe", || Box::new(BbFrameAdapt::default()));
    r.add("ipcast.alp", || Box::new(AlpComponent::default()));
    r.add("ipcast.ip", || Box::new(IpComponent::default()));
    r.add("ipcast.route", || Box::new(RouteComponent::default()));
}

/// The data PLP and the common PLP of its group that a receiver decodes. A receiver sends one when either changes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PlpPair {
    /// The data PLP.
    pub plp: Option<u8>,
    /// Its common PLP.
    pub common: Option<u8>,
}

impl Item for PlpPair {
    const KIND: &'static str = "ipcast.plps";
}

/// An IP packet (IPv4 or IPv6) out of an encapsulation, with where it came from.
#[derive(Clone, Debug, PartialEq)]
pub struct IpPacket {
    /// The packet.
    pub packet: Packet,
    /// The input stream (ISI) or PLP it came in.
    pub stream: u8,
    /// The GSE label (6, 3 or 0 bytes used).
    pub label: [u8; 6],
    /// The bytes of `label` used.
    pub label_len: u8,
}

impl Item for IpPacket {
    const KIND: &'static str = "ip.packet";
}

/// A UDP datagram made whole, with the result of its checksum.
#[derive(Clone, Debug, PartialEq)]
pub struct UdpDatagram {
    /// The source address.
    pub src: IpAddr,
    /// The destination address.
    pub dst: IpAddr,
    /// The source port.
    pub src_port: u16,
    /// The destination port.
    pub dst_port: u16,
    /// What the UDP checksum said (`None`: not sent).
    pub checksum_ok: Option<bool>,
    /// The payload.
    pub payload: Packet,
}

impl Item for UdpDatagram {
    const KIND: &'static str = "ip.udp";
}

/// A whole ROUTE object.
#[derive(Clone, Debug, PartialEq)]
pub struct RouteObjectItem {
    /// The destination address of its session.
    pub dst: IpAddr,
    /// The destination UDP port of its session.
    pub dst_port: u16,
    /// The TSI.
    pub tsi: u64,
    /// The TOI.
    pub toi: u64,
    /// The codepoint.
    pub codepoint: u8,
    /// The object.
    pub data: Packet,
}

impl Item for RouteObjectItem {
    const KIND: &'static str = "route.object";
}

/// An ALP link layer signalling packet.
#[derive(Clone, Debug, PartialEq)]
pub struct AlpSignalling {
    /// signaling_type (1 link mapping table, 2 ROHC-U description table).
    pub kind: u8,
    /// signaling_version.
    pub version: u8,
    /// signaling_format (0 binary, 1 XML, 2 JSON).
    pub format: u8,
    /// signaling_encoding (0 none, 1 DEFLATE).
    pub encoding: u8,
    /// The signalling data.
    pub payload: Packet,
}

impl Item for AlpSignalling {
    const KIND: &'static str = "alp.signalling";
}

/// PCAP file bytes of one IP packet: its record, with the file header in front when it starts a capture.
#[derive(Clone, Debug, PartialEq)]
pub struct PcapRecord {
    /// The bytes to append to the file.
    pub data: Packet,
    /// True when `data` starts with the PCAP file header: the host starts a new file.
    pub starts_file: bool,
}

impl Item for PcapRecord {
    const KIND: &'static str = "ipcast.pcap";
}

/// The result fields of `ipcast.bbframe`: the counts of the mode adaptation.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct BbFrameFields {
    /// BBFRAMEs with a good header.
    #[field(shared = keys::FRAMES_OK)]
    pub frames_ok: i64,
    /// BBFRAMEs with a bad header.
    #[field(shared = keys::FRAMES_BAD)]
    pub frames_bad: i64,
    /// Transport packets of the chosen stream.
    #[field(label = "TS packets")]
    pub ts_packets: i64,
    /// User packets with a bad CRC-8.
    #[field(label = "CRC errors")]
    pub crc_errors: i64,
    /// GSE PDUs of every stream.
    #[field(label = "PDUs")]
    pub pdus: i64,
    /// GSE PDUs that carry no IP.
    #[field(label = "Non-IP PDUs")]
    pub non_ip: i64,
}

/// The result fields of `ipcast.alp`: the counts of the link layer.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct AlpFields {
    /// ALP packets read.
    #[field(label = "ALP packets")]
    pub alp_packets: i64,
    /// ALP packets dropped.
    #[field(label = "ALP packets dropped")]
    pub alp_dropped: i64,
    /// ALP packets of IP with compressed headers.
    #[field(label = "Compressed IP packets")]
    pub compressed_ip: i64,
}

/// The result fields of `ipcast.ip`: the counts of the IP layer.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct IpFields {
    /// IP packets read.
    #[field(label = "IP packets")]
    pub ip_packets: i64,
    /// IP packets refused.
    #[field(label = "Bad IP packets")]
    pub ip_bad: i64,
    /// Datagrams put together from fragments.
    #[field(label = "Reassembled")]
    pub reassembled: i64,
    /// Flows seen.
    #[field(label = "Flows")]
    pub flows: i64,
    /// Flows whose payload is encrypted.
    #[field(label = "Encrypted flows")]
    pub encrypted_flows: i64,
}

/// The result fields of `ipcast.route`: the counts of the object delivery.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct RouteFields {
    /// Objects received whole.
    #[field(label = "Objects")]
    pub objects: i64,
    /// Objects dropped.
    #[field(label = "Objects dropped")]
    pub objects_dropped: i64,
}

/// The interface of this crate, which stacks of several families show: the result sets of its components. A family
/// whose stacks list one of them names this interface with `Family::uses`, and adds the set of a component of its
/// graph with [`describe`].
pub fn interface() -> &'static Family {
    static SHARED: OnceLock<Family> = OnceLock::new();
    SHARED.get_or_init(|| {
        Family::shared("ipcast", 1, 0).stack(
            StackShape::new("ipcast")
                .component("bbframe", |c| c.fields::<BbFrameFields>())
                .component("alp", |c| c.fields::<AlpFields>())
                .component("ip", |c| c.fields::<IpFields>())
                .component("route", |c| c.fields::<RouteFields>()),
        )
    })
}

/// `shape` with the result set `F` of the component of this crate that the graph of the stack holds under the name
/// `component`: [`BbFrameFields`], [`AlpFields`], [`IpFields`] or [`RouteFields`]. A field whose key a result set of
/// the stack declares is the field of the stack; every other field has the key `<component>.<key>`. List the result
/// sets of the stack first.
#[must_use]
pub fn describe<F: Fields>(shape: StackShape, component: &'static str) -> StackShape {
    shape.component(component, |c| c.fields::<F>().shares_declared())
}

/// A count as a whole number of a result field.
fn count(v: u64) -> i64 {
    v.min(i64::MAX as u64) as i64
}

/// The log line while ALP packets of compressed IP arrive.
pub const COMPRESSED_IP_NOTE: &str =
    "link-layer packets carry compressed IP headers; services sent that way are not decoded";
/// Seconds of signal time without compressed IP after which the note is given again.
const COMPRESSED_IP_QUIET_S: f64 = 10.0;
/// True when the input item at `k` of this call carries a `Discontinuity` tag (items were lost before it).
fn lost_before(io: &Io, k: usize) -> bool {
    let input = io.input(0);
    let at = input.first() + k as u64;
    input
        .tags()
        .iter()
        .any(|t| t.at == at && matches!(t.kind, TagKind::Discontinuity { .. }))
}

/// A rate-free stream of typed items.
fn items_format<T: Item>() -> Format {
    Format::new(Kind::typed::<T>(), 0.0)
}

fn ip_item(
    pool: &mut PacketPool,
    bytes: &[u8],
    time: f64,
    stream: u8,
    label: Label,
) -> Timed<IpPacket> {
    let mut l = [0u8; 6];
    let lb = label.bytes();
    l[..lb.len()].copy_from_slice(lb);
    Timed {
        time,
        value: IpPacket {
            packet: pool.make(bytes.len(), time, |b| b.copy_from_slice(bytes)),
            stream,
            label: l,
            label_len: lb.len() as u8,
        },
    }
}

/// `ipcast.bbframe`: DVB-S2, DVB-T2 or DVB-C2 mode adaptation. Input `in`: one descrambled BBFRAME per `Bytes` packet. Input
/// `plps` (optional): [`PlpPair`] items. Output `ts`: the transport packets of the chosen stream, as [`TsPacket`] items. Output
/// `ip`: the IP packets in the GSE of every stream.
///
/// Without `plps` the chosen stream is the `stream` setting (or the first transport stream seen). With `plps` the frames whose
/// MATYPE-2 names the common PLP feed the common PLP and every other transport stream frame feeds the data PLP; `ts` carries
/// the data PLP with its common PLP put back ([`CommonMerge`]).
///
/// A `Discontinuity` tag on the input (frames lost before that frame) and a frame with a bad header break the packet chains.
/// The first transport packet after a break, or after a lost chain that SYNCD found again, has a `Discontinuity` tag.
pub struct BbFrameAdapt {
    system: SystemChoice,
    stream: i64,
    chosen: Option<u8>,
    deframer: UpDeframer,
    pair: Option<PlpPair>,
    common: UpDeframer,
    merge: CommonMerge,
    gse: Vec<(u8, GseReader)>,
    pool: PacketPool,
    ts: TsOut,
    frames_ok: u64,
    frames_bad: u64,
    non_ip: u64,
    loss: bool,
    results: Published,
}

/// The most input streams whose GSE is read at once.
const MAX_STREAMS: usize = 16;

impl Default for BbFrameAdapt {
    fn default() -> Self {
        BbFrameAdapt {
            system: SYSTEM.default,
            stream: STREAM.default,
            chosen: None,
            deframer: UpDeframer::new(),
            pair: None,
            common: UpDeframer::new(),
            merge: CommonMerge::default(),
            gse: Vec::with_capacity(MAX_STREAMS),
            pool: PacketPool::default(),
            ts: TsOut::default(),
            frames_ok: 0,
            frames_bad: 0,
            non_ip: 0,
            loss: false,
            results: Published::new(BbFrameFields::SHAPE.fields),
        }
    }
}

impl BbFrameAdapt {
    fn publish(&mut self) {
        self.results.put(&BbFrameFields {
            frames_ok: count(self.frames_ok),
            frames_bad: count(self.frames_bad),
            ts_packets: count(self.deframer.stats.packets),
            crc_errors: count(self.deframer.stats.crc_errors),
            pdus: count(self.gse.iter().map(|g| g.1.stats.pdus).sum()),
            non_ip: count(self.non_ip),
        });
    }

    /// Breaks every chain: the transport packets and the GSE packets cut across frames.
    fn break_chains(&mut self) {
        self.deframer.reset();
        self.common.reset();
        self.merge.reset();
        for g in &mut self.gse {
            g.1.reset();
        }
        self.loss = true;
    }

    /// One frame at signal time `time`.
    fn frame(&mut self, io: &mut Io, frame: &[u8], time: f64) {
        let Ok(h) = BbHeader::parse(frame) else {
            self.frames_bad += 1;
            self.break_chains();
            return;
        };
        let Some(df) = h
            .data_field_len()
            .ok()
            .and_then(|n| frame.get(BBHEADER_LEN..BBHEADER_LEN + n))
        else {
            self.frames_bad += 1;
            self.break_chains();
            return;
        };
        self.frames_ok += 1;
        let id = h.stream_id;
        if h.carries_gse() {
            let k = match self.gse.iter().position(|g| g.0 == id) {
                Some(k) => k,
                None if self.gse.len() < MAX_STREAMS => {
                    self.gse.push((id, GseReader::default()));
                    self.gse.len() - 1
                }
                None => return,
            };
            let sliced = h.gse_sliced(self.system.system());
            let (pool, non_ip) = (&mut self.pool, &mut self.non_ip);
            let out = io.items_out::<IpPacket>(1);
            let mut each = |p: crate::gse::GsePdu<'_>| match p.protocol {
                0x0800 | 0x86DD if !p.ext.bridged => {
                    out.push(ip_item(pool, p.pdu, time, id, p.label))
                }
                _ => *non_ip += 1,
            };
            if sliced {
                self.gse[k].1.push_sliced(df, h.syncd, &mut each);
            } else {
                self.gse[k].1.push(df, &mut each);
            }
            return;
        }
        if h.format != StreamFormat::Transport {
            return;
        }
        if let Some(pair) = self.pair {
            self.merged_frame(io, &h, df, time, pair);
            return;
        }
        let chosen = *self.chosen.get_or_insert(if self.stream >= 0 {
            self.stream as u8
        } else {
            id
        });
        if chosen != id {
            return;
        }
        let before = (self.deframer.stats.resyncs, self.deframer.stats.refused);
        let out = &mut self.ts;
        let mut ts = std::mem::take(io.items_out::<TsPacket>(0));
        let start = ts.len();
        let r = self.deframer.push(&h, df, &mut |p| {
            // The deframer sets the transport_error_indicator of a packet with a wrong CRC-8 itself.
            if let Ok(b) = p.bytes.try_into() {
                out.push(&mut ts, b, time, false);
            }
        });
        let broke =
            r.is_err() || (self.deframer.stats.resyncs, self.deframer.stats.refused) != before;
        self.finish_ts(io, ts, start, broke);
    }

    /// One transport stream frame of the data PLP or the common PLP of `pair`, through the merge.
    fn merged_frame(&mut self, io: &mut Io, h: &BbHeader, df: &[u8], time: f64, pair: PlpPair) {
        let to_common = pair
            .common
            .is_some_and(|c| c == h.stream_id && Some(c) != pair.plp);
        let merge = &mut self.merge;
        let d = if to_common {
            &mut self.common
        } else {
            &mut self.deframer
        };
        let before = (d.stats.resyncs, d.stats.refused);
        let r = d.push(h, df, &mut |u| {
            if to_common {
                merge.push_common(u.bytes, u.crc_ok);
            } else {
                merge.push_data(u.bytes, u.crc_ok);
            }
        });
        let broke = r.is_err() || (d.stats.resyncs, d.stats.refused) != before;
        let out = &mut self.ts;
        let mut ts = std::mem::take(io.items_out::<TsPacket>(0));
        let start = ts.len();
        let mut lost = false;
        // The deframer has set the transport_error_indicator of a packet with a wrong CRC-8.
        self.merge
            .drain(|p, _| lost |= !out.push(&mut ts, p, time, false));
        self.finish_ts(io, ts, start, broke || lost);
    }

    /// Puts back the transport packets of one frame, the first one tagged after a break.
    fn finish_ts(&mut self, io: &mut Io, ts: Vec<Timed<TsPacket>>, start: usize, broke: bool) {
        if broke {
            self.loss = true;
        }
        let tag = self.loss && ts.len() > start;
        *io.items_out::<TsPacket>(0) = ts;
        if tag {
            io.output(0)
                .tag_here(start, TagKind::Discontinuity { lost: 0 });
            self.loss = false;
        }
    }
}

sdrkit_core::choice_enum! {
    /// The standard of the BBHEADER that `ipcast.bbframe` reads.
    pub enum SystemChoice {
        /// DVB-S2 and DVB-S2X.
        DvbS2 = ("dvb_s2", "DVB-S2"),
        /// DVB-T2 and DVB-C2 (the same header).
        DvbT2 = ("dvb_t2", "DVB-T2"),
    }
}

impl SystemChoice {
    /// The system of [`crate::bbheader`].
    pub const fn system(self) -> System {
        match self {
            SystemChoice::DvbS2 => System::DvbS2,
            SystemChoice::DvbT2 => System::DvbT2,
        }
    }
}

const SYSTEM: Setting<field::Choice<SystemChoice>> =
    Setting::choice("system", "System", SystemChoice::DvbS2)
        .help("Which standard's BBHEADER the frames carry.");
const STREAM: Setting<field::I64> = Setting::new("stream", "Stream", "", -1)
    .range(-1.0, 255.0, 1.0)
    .help("The input stream (ISI) or PLP whose transport stream goes out. -1: the first seen.");

sdrkit_core::settings! {
    /// The settings of [`BbFrameAdapt`].
    pub enum BbFrameAdaptSetting {
        /// The standard of the BBHEADER.
        System = SYSTEM,
        /// The stream whose transport stream goes out.
        Stream = STREAM,
    }
}

impl Component for BbFrameAdapt {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "ipcast.bbframe",
            vec![
                Port::bytes("in"),
                Port::typed::<PlpPair>("plps")
                    .optional("the transport stream of the `stream` setting, with no common PLP"),
            ],
            vec![
                ts_stream::port("ts").rate_from(RateFrom::Fixed(0.0)),
                Port::typed::<IpPacket>("ip").rate_from(RateFrom::Fixed(0.0)),
            ],
        )
        .manual_tags()
        .with_settings(BbFrameAdaptSetting::schema())
    }
    fn configure(&mut self, _inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.break_chains();
        self.chosen = None;
        self.pair = None;
        self.merge.set_common(false);
        Ok(vec![items_format::<TsPacket>(), items_format::<IpPacket>()])
    }
    fn process(&mut self, io: &mut Io) {
        if io.is_connected(1) {
            let mut next = self.pair;
            for s in io.items::<PlpPair>(1) {
                next = Some(s.value);
            }
            if next != self.pair {
                self.pair = next;
                self.merge
                    .set_common(next.is_some_and(|p| p.common.is_some()));
                self.break_chains();
            }
        }
        let input = io.bytes(0);
        if input.is_empty() {
            return;
        }
        for (k, f) in input.iter().enumerate() {
            if lost_before(io, k) {
                self.break_chains();
            }
            self.frame(io, f.bytes(), f.time);
        }
        self.publish();
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match BbFrameAdaptSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            BbFrameAdaptSetting::System => self.system = SYSTEM.read(id, v)?,
            BbFrameAdaptSetting::Stream => {
                self.stream = STREAM.read(id, v)?;
                self.chosen = None;
                self.break_chains();
            }
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([
            (
                BbFrameAdaptSetting::System,
                Value::Choice(self.system.index()),
            ),
            (BbFrameAdaptSetting::Stream, Value::I64(self.stream)),
        ])
    }
    fn forget(&mut self, _: &Kept) {
        self.break_chains();
        self.deframer = UpDeframer::new();
        self.common = UpDeframer::new();
        self.gse.clear();
        self.chosen = None;
        self.frames_ok = 0;
        self.frames_bad = 0;
        self.non_ip = 0;
        self.publish();
    }
    fn results(&self) -> Option<&Results> {
        Some(self.results.results())
    }
}

/// `ipcast.alp`: ATSC 3.0 link layer. Input: one descrambled baseband packet per `Bytes` packet. Outputs: `ts` (MPEG-2 TS
/// carried in ALP, as [`TsPacket`] items), `ip` (IPv4 packets) and `signalling` (link layer signalling).
///
/// A `Discontinuity` tag on the input (baseband packets lost before that one) starts the reader again at the next pointer. The
/// first transport packet after a loss, or after a packet the reader dropped, has a `Discontinuity` tag.
///
/// Packets of compressed IP (type 2, ROHC-U) are counted, not decoded; the log has [`COMPRESSED_IP_NOTE`] when they start to
/// arrive, and again after 10 s of signal time without them.
pub struct AlpComponent {
    reader: AlpReader,
    pool: PacketPool,
    ts: TsOut,
    loss: bool,
    compressed_at: Option<f64>,
    results: Published,
}

impl Default for AlpComponent {
    fn default() -> Self {
        AlpComponent {
            reader: AlpReader::default(),
            pool: PacketPool::default(),
            ts: TsOut::default(),
            loss: false,
            compressed_at: None,
            results: Published::new(AlpFields::SHAPE.fields),
        }
    }
}

impl AlpComponent {
    fn publish(&mut self) {
        let s = &self.reader.stats;
        self.results.put(&AlpFields {
            alp_packets: count(s.packets),
            alp_dropped: count(s.dropped),
            compressed_ip: count(s.compressed_ip),
        });
    }
}

impl Component for AlpComponent {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "ipcast.alp",
            vec![Port::bytes("in")],
            vec![
                ts_stream::port("ts").rate_from(RateFrom::Fixed(0.0)),
                Port::typed::<IpPacket>("ip").rate_from(RateFrom::Fixed(0.0)),
                Port::typed::<AlpSignalling>("signalling").rate_from(RateFrom::Fixed(0.0)),
            ],
        )
        .manual_tags()
    }
    fn configure(&mut self, _inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.reader.gap();
        self.loss = true;
        self.compressed_at = None;
        Ok(vec![
            items_format::<TsPacket>(),
            items_format::<IpPacket>(),
            items_format::<AlpSignalling>(),
        ])
    }
    fn process(&mut self, io: &mut Io) {
        let input = io.bytes(0);
        if input.is_empty() {
            return;
        }
        for (k, b) in input.iter().enumerate() {
            if lost_before(io, k) {
                self.reader.gap();
                self.loss = true;
            }
            let time = b.time;
            let before = (self.reader.stats.dropped, self.reader.stats.bad_headers);
            let compressed = self.reader.stats.compressed_ip;
            let (pool, ts_out) = (&mut self.pool, &mut self.ts);
            let mut ts = std::mem::take(io.items_out::<TsPacket>(0));
            let mut ip = std::mem::take(io.items_out::<IpPacket>(1));
            let mut sig = std::mem::take(io.items_out::<AlpSignalling>(2));
            let start = ts.len();
            self.reader.push_bbp(b.bytes(), &mut |e| match e {
                AlpEvent::Ts { packet, .. } => {
                    ts_out.push(&mut ts, packet, time, false);
                }
                AlpEvent::Packet(p) => match (p.kind, p.signalling) {
                    (PacketType::Ipv4, _) => {
                        ip.push(ip_item(pool, p.payload, time, 0, Label::Broadcast))
                    }
                    (PacketType::Signalling, Some(s)) => sig.push(Timed {
                        time,
                        value: AlpSignalling {
                            kind: s.kind,
                            version: s.version,
                            format: s.format,
                            encoding: s.encoding,
                            payload: pool
                                .make(p.payload.len(), time, |o| o.copy_from_slice(p.payload)),
                        },
                    }),
                    _ => {}
                },
            });
            if (self.reader.stats.dropped, self.reader.stats.bad_headers) != before {
                self.loss = true;
            }
            if self.reader.stats.compressed_ip != compressed {
                if self
                    .compressed_at
                    .is_none_or(|t| time - t > COMPRESSED_IP_QUIET_S)
                {
                    self.results.results_mut().log(COMPRESSED_IP_NOTE);
                }
                self.compressed_at = Some(time);
            }
            if self.loss && ts.len() > start {
                io.output(0)
                    .tag_here(start, TagKind::Discontinuity { lost: 0 });
                self.loss = false;
            }
            *io.items_out::<TsPacket>(0) = ts;
            *io.items_out::<IpPacket>(1) = ip;
            *io.items_out::<AlpSignalling>(2) = sig;
        }
        self.publish();
    }
    fn forget(&mut self, _: &Kept) {
        self.reader = AlpReader::default();
        self.loss = true;
        self.compressed_at = None;
        self.publish();
    }
    fn apply(&mut self, id: SettingId, _: &Value) -> Result<Applied, SettingError> {
        Err(SettingError::NoSuchId { id })
    }
    fn values(&self) -> Values {
        no_values()
    }
    fn results(&self) -> Option<&Results> {
        Some(self.results.results())
    }
}

/// `ipcast.ip`: IPv4 reassembly, UDP checksums and flow statistics. Input: `ip.packet`. Output `udp`: whole UDP datagrams.
/// Output `pcap`: a [`PcapRecord`] of every IP packet, when the `pcap` setting is on. The first record after the setting is
/// turned on starts the file.
pub struct IpComponent {
    reasm: Reassembler,
    flows: FlowTable,
    pool: PacketPool,
    pcap: bool,
    pcap_started: bool,
    scratch: Vec<u8>,
    packets: u64,
    results: Published,
}

impl Default for IpComponent {
    fn default() -> Self {
        IpComponent {
            reasm: Reassembler::default(),
            flows: FlowTable::new(256),
            pool: PacketPool::default(),
            pcap: false,
            pcap_started: false,
            scratch: Vec::with_capacity(70_000),
            packets: 0,
            results: Published::new(IpFields::SHAPE.fields),
        }
    }
}

const PCAP: Setting<field::Bool> = Setting::new("pcap", "PCAP output", "", false)
    .help("Send every IP packet as PCAP file bytes on the pcap output. The host writes the file.");

sdrkit_core::settings! {
    /// The settings of [`IpComponent`].
    pub enum IpComponentSetting {
        /// PCAP output.
        Pcap = PCAP,
    }
}

impl IpComponent {
    fn publish(&mut self) {
        self.results.put(&IpFields {
            ip_packets: count(self.packets),
            ip_bad: count(self.reasm.stats.refused),
            reassembled: count(self.reasm.stats.reassembled),
            flows: self.flows.flows().len() as i64,
            encrypted_flows: self.flows.encrypted_flows() as i64,
        });
    }
}

impl Component for IpComponent {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "ipcast.ip",
            vec![Port::typed::<IpPacket>("in")],
            vec![
                Port::typed::<UdpDatagram>("udp").rate_from(RateFrom::Fixed(0.0)),
                Port::typed::<PcapRecord>("pcap").rate_from(RateFrom::Fixed(0.0)),
            ],
        )
        .with_settings(IpComponentSetting::schema())
    }
    fn configure(&mut self, _inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        Ok(vec![
            items_format::<UdpDatagram>(),
            items_format::<PcapRecord>(),
        ])
    }
    fn process(&mut self, io: &mut Io) {
        let input = io.items::<IpPacket>(0);
        if input.is_empty() {
            return;
        }
        let writer = PcapWriter::new(LinkType::Raw);
        for p in input {
            let bytes = p.value.packet.bytes();
            let time = p.time;
            self.packets += 1;
            if self.pcap {
                self.scratch.clear();
                let starts_file = !self.pcap_started;
                if starts_file {
                    writer.file_header(&mut self.scratch);
                }
                if writer
                    .record(time.max(0.0), bytes, &mut self.scratch)
                    .is_ok()
                {
                    self.pcap_started = true;
                    let s = &self.scratch;
                    let data = self.pool.make(s.len(), time, |b| b.copy_from_slice(s));
                    io.items_out::<PcapRecord>(1).push(Timed {
                        time,
                        value: PcapRecord { data, starts_file },
                    });
                }
            }
            match bytes.first().map(|b| b >> 4) {
                Some(4) => {
                    let (flows, pool) = (&mut self.flows, &mut self.pool);
                    let out = io.items_out::<UdpDatagram>(0);
                    // The reassembler counts a refused packet in its stats.
                    let _ = self.reasm.push(bytes, time, &mut |d| {
                        flows.datagram(&d, time);
                        if let Some(Ok(u)) = d.udp() {
                            out.push(Timed {
                                time,
                                value: UdpDatagram {
                                    src: u.src,
                                    dst: u.dst,
                                    src_port: u.src_port,
                                    dst_port: u.dst_port,
                                    checksum_ok: u.checksum_ok,
                                    payload: pool.make(u.payload.len(), time, |b| {
                                        b.copy_from_slice(u.payload)
                                    }),
                                },
                            });
                        }
                    });
                }
                Some(6) => {
                    self.flows.ipv6(bytes, time);
                    if let Ok(v6) = crate::ip::Ipv6::parse(bytes) {
                        if v6.protocol == crate::ip::UDP && !v6.fragment {
                            if let Ok(u) =
                                crate::ip::Udp::parse(v6.src.into(), v6.dst.into(), v6.payload)
                            {
                                let pool = &mut self.pool;
                                io.items_out::<UdpDatagram>(0).push(Timed {
                                    time,
                                    value: UdpDatagram {
                                        src: u.src,
                                        dst: u.dst,
                                        src_port: u.src_port,
                                        dst_port: u.dst_port,
                                        checksum_ok: u.checksum_ok,
                                        payload: pool.make(u.payload.len(), time, |b| {
                                            b.copy_from_slice(u.payload)
                                        }),
                                    },
                                });
                            }
                        }
                    }
                }
                _ => self.reasm.stats.refused += 1,
            }
        }
        self.publish();
    }
    fn forget(&mut self, _: &Kept) {
        self.reasm = Reassembler::default();
        self.flows.clear();
        self.packets = 0;
        self.publish();
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match IpComponentSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            IpComponentSetting::Pcap => {
                self.pcap = PCAP.read(id, v)?;
                if !self.pcap {
                    self.pcap_started = false;
                }
            }
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([(IpComponentSetting::Pcap, Value::Bool(self.pcap))])
    }
    fn results(&self) -> Option<&Results> {
        Some(self.results.results())
    }
}

/// `ipcast.route`: ROUTE objects from UDP datagrams. Setting `port`: only datagrams to this destination port (0: all).
pub struct RouteComponent {
    asm: RouteAssembler,
    pool: PacketPool,
    port: u16,
    results: Published,
}

impl Default for RouteComponent {
    fn default() -> Self {
        RouteComponent {
            asm: RouteAssembler::default(),
            pool: PacketPool::default(),
            port: ROUTE_PORT.default as u16,
            results: Published::new(RouteFields::SHAPE.fields),
        }
    }
}

const ROUTE_PORT: Setting<field::I64> = Setting::new("port", "UDP port", "", 0)
    .range(0.0, 65535.0, 1.0)
    .help("Only datagrams to this destination port. 0: all.");

sdrkit_core::settings! {
    /// The settings of [`RouteComponent`].
    pub enum RouteComponentSetting {
        /// The UDP port.
        Port = ROUTE_PORT,
    }
}

impl RouteComponent {
    fn publish(&mut self) {
        self.results.put(&RouteFields {
            objects: count(self.asm.stats.objects),
            objects_dropped: count(self.asm.stats.dropped),
        });
    }
}

impl Component for RouteComponent {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "ipcast.route",
            vec![Port::typed::<UdpDatagram>("in")],
            vec![Port::typed::<RouteObjectItem>("out").rate_from(RateFrom::Fixed(0.0))],
        )
        .with_settings(RouteComponentSetting::schema())
    }
    fn configure(&mut self, _inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        Ok(vec![items_format::<RouteObjectItem>()])
    }
    fn process(&mut self, io: &mut Io) {
        let input = io.items::<UdpDatagram>(0);
        if input.is_empty() {
            return;
        }
        for d in input {
            if self.port != 0 && d.value.dst_port != self.port {
                continue;
            }
            let time = d.time;
            let pool = &mut self.pool;
            let out = io.items_out::<RouteObjectItem>(0);
            // The assembler counts a refused packet in its stats.
            let dst = (d.value.dst, d.value.dst_port);
            let _ = self.asm.push(d.value.payload.bytes(), dst, time, &mut |o| {
                out.push(Timed {
                    time,
                    value: RouteObjectItem {
                        dst: o.dst.0,
                        dst_port: o.dst.1,
                        tsi: o.tsi,
                        toi: o.toi,
                        codepoint: o.codepoint,
                        data: pool.make(o.data.len(), time, |b| b.copy_from_slice(o.data)),
                    },
                });
            });
        }
        self.publish();
    }
    fn forget(&mut self, _: &Kept) {
        self.asm = RouteAssembler::default();
        self.publish();
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match RouteComponentSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            RouteComponentSetting::Port => self.port = ROUTE_PORT.read(id, v)? as u16,
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([(
            RouteComponentSetting::Port,
            Value::I64(i64::from(self.port)),
        )])
    }
    fn results(&self) -> Option<&Results> {
        Some(self.results.results())
    }
}
