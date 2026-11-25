//! File broadcasts on a DVB-S2 carrier (`sat_filecast`, submodes `himawaricast` and `geonetcast`): the transport stream of
//! the DVB-S2 receiver, the datagram sections (MPE) of one PID, IPv4 UDP datagrams, the carousel file broadcast that cuts
//! files into numbered parts, and the files. HimawariCast files are bzip2 data; its HRIT segments become full-disc pictures,
//! one per channel. The stack never writes a file: each leaves as a `satimg.file` item that the host stores.
//!
//! No public document was read for the file broadcast: its packet layout, the part sizes, the PIDs and the 45-character tag
//! at the end of GEONETCAST names are not confirmed by a public document.
//!
//! Standard: ETSI EN 302 307-1 (DVB-S2); ETSI EN 301 192 clause 7 (MPE); RFC 791, RFC 768; the bzip2 file format; CGMS
//! LRIT/HRIT global specification and the JMA HRIT mission specific implementation (headers).

use super::s2::{
    self, attach, link_of, routes, Dvbs2Parts, LinkState, CARRIER_OFFSET, FRAME_LENGTH, MODCOD,
    MODCODS, MODCOD_RULE, PILOTS_RULE, PL_SCRAMBLING, TEST_SIGNAL,
};
use crate::components::last;
use crate::items::{FileKind, SatFile};
use crate::typed::{
    FilecastFields, FilecastPictureRow, RecentFileRow, FILECAST_PICTURES, RECENT_FILES,
};
use crate::REPORT_S;
use sdrkit_codec_image::{PixelFormat, Raster};
use sdrkit_core::command::{Command, CommandError, CommandId, CommandSet, Commands};
use sdrkit_core::interface::Fields as _;
use sdrkit_core::media::Picture;
use sdrkit_core::publish::{field, Published};
use sdrkit_core::{
    AutoGain, Choice, ConfigError, GenConfig, Generator, LockProfile, Receiver, Record, Results,
    ResultsDecl, Setting, SettingError, SettingId, SettingSet, StackConfig, StackDescriptor,
    SubMode, Tuning, Value, Values,
};
use sdrkit_graph::{
    values_of, Applied, Catalog, Component, ComponentInfo, Format, GraphBuilder, GraphError, Io,
    Item, Kind, Pool, Port, RateFrom, RunContext, StackGraph, TagKind, Ticket, Timed,
};
use sdrkit_proto_ccsds::xrit::headers::{parse as parse_headers, Builder, Compression, Dialect};
use sdrkit_proto_ipcast::filecast::{
    clean_name, head_packet, part_packet, tail_packet, FileAssembler, FilecastConfig, FinishedFile,
};
use sdrkit_proto_ipcast::mpe::{section_packets, write_ipv4_udp_plain, write_section, MpeReader};
use sdrkit_proto_mpegts::source::PacketSource;
use sdrkit_proto_mpegts::stream::TsPacket;
use sdrkit_proto_dvbs2::settings::{RollOffChoice, ISI};
use std::collections::{BTreeMap, VecDeque};
use std::fmt::Write as _;
use std::net::Ipv4Addr;
use std::sync::Arc;

/// The stack id.
pub const ID: &str = "sat_filecast";
/// The submode of HimawariCast.
pub const HIMAWARICAST: &str = "himawaricast";
/// The submode of GEONETCAST Americas.
pub const GEONETCAST: &str = "geonetcast";
/// The input rate the stack asks for (HimawariCast; GEONETCAST at its own rate needs 52 Msps).
pub const ASKED_RATE: f64 = 6e6;
/// The symbol rate of the test signal when the settings give none.
pub const TEST_SYMBOL_RATE: f64 = 2_586_148.0;
/// The characters of the tag at the end of a GEONETCAST name (not confirmed by a public document): the default of
/// [`NAME_TAG`].
pub const GEONETCAST_TAG: usize = 45;
/// The largest output of one bzip2 file.
pub const MAX_BZIP2_OUT: usize = 256 << 20;
/// The most bytes of files waiting for the worker.
pub const MAX_QUEUE: usize = 512 << 20;
/// Rows of the table of recent files.
pub const RECENT: usize = 60;
/// The most pixels of one HRIT picture (a segment that would make a larger picture counts as bad).
pub const MAX_PICTURE: u64 = 1 << 28;

sdrkit_core::choice_enum! {
    /// The file broadcast.
    #[derive(Default)]
    pub enum Service {
        /// HimawariCast.
        #[default]
        Himawaricast = ("himawaricast", "HimawariCast"),
        /// GEONETCAST Americas.
        Geonetcast = ("geonetcast", "GEONETCAST Americas"),
    }
}

impl Service {
    /// The service of a submode id; HimawariCast for `None`.
    pub fn of(submode: Option<&str>) -> Service {
        match submode {
            Some(GEONETCAST) => Service::Geonetcast,
            _ => Service::Himawaricast,
        }
    }
    /// The MODCOD of its test signal.
    pub fn test_modcod(self) -> u8 {
        match self {
            Service::Himawaricast => 6,
            Service::Geonetcast => 15,
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------------------------
// HRIT pictures.

/// A HimawariCast segment name taken apart: "IMG_DK01<channel>_<YYYYMMDDhhmm>_<NNN>".
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SegmentName {
    /// The channel ("IR1", "B13").
    pub channel: String,
    /// The time stamp, 12 digits.
    pub stamp: String,
    /// The segment number, from 1.
    pub number: u16,
}

/// Reads a segment name; `None` when `name` is not one. A dot and what follows it after the number is allowed.
pub fn segment_name(name: &str) -> Option<SegmentName> {
    let mut f = name.split('_');
    let (img, dk, stamp, num) = (f.next()?, f.next()?, f.next()?, f.next()?);
    if f.next().is_some() || img != "IMG" {
        return None;
    }
    let channel = dk.strip_prefix("DK01").filter(|c| !c.is_empty())?;
    if stamp.len() != 12 || !stamp.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let digits = num.split('.').next()?;
    if digits.len() != 3 || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let number: u16 = digits.parse().ok().filter(|&n| n >= 1)?;
    Some(SegmentName {
        channel: channel.to_owned(),
        stamp: stamp.to_owned(),
        number,
    })
}

/// A picture of one channel while its segments arrive.
#[derive(Clone, Debug, PartialEq)]
pub struct Mosaic {
    /// The channel.
    pub channel: String,
    /// The time stamp.
    pub stamp: String,
    /// Columns.
    pub width: u32,
    /// Lines of one segment.
    pub segment_lines: u32,
    /// Segments in the picture.
    pub total: u32,
    /// Which segments landed.
    pub landed: Vec<bool>,
    /// 8-bit samples, row by row, `width` x `segment_lines` x `total`.
    pub pixels: Vec<u8>,
}

impl Mosaic {
    /// Segments landed.
    pub fn received(&self) -> u32 {
        self.landed.iter().filter(|&&l| l).count() as u32
    }
    /// Its name: "<channel>_<stamp>", with "_partial" when `partial`.
    pub fn name(&self, partial: bool) -> String {
        let mut n = format!("{}_{}", self.channel, self.stamp);
        if partial {
            n.push_str("_partial");
        }
        n
    }
    /// The picture item with the number `id`. It is finished when `last`, else it can grow.
    pub fn picture(&self, partial: bool, id: u32, last: bool) -> Option<Picture> {
        let state = if last {
            sdrkit_core::PictureState::Finished
        } else {
            sdrkit_core::PictureState::Growing
        };
        Raster::from_vec(
            PixelFormat::Grey8,
            self.width,
            self.segment_lines * self.total,
            self.pixels.clone(),
        )
        .ok()
        .map(|r| r.to_picture(&self.name(partial), id, state))
    }
}

/// What [`HritMosaics::push`] gives out.
#[derive(Clone, Debug, PartialEq)]
pub enum MosaicOut {
    /// The picture so far, after a segment landed.
    Progress(Picture),
    /// A finished picture (all segments) or one pushed out by another (partial).
    Done {
        /// The picture.
        picture: Picture,
        /// All segments landed.
        complete: bool,
    },
}

/// One picture per channel, assembled from HRIT segment files. See [`HritMosaics::push`].
#[derive(Clone, Debug, Default)]
pub struct HritMosaics {
    /// The pictures in progress by channel.
    pub open: BTreeMap<String, Mosaic>,
    /// The newest state of each channel: (stamp, width, height, received, total).
    pub rows: BTreeMap<String, (String, u32, u32, u32, u32)>,
    /// Segments refused: no image structure record, a compression, sizes out of range, or too few bytes.
    pub bad: u64,
    /// Pictures finished.
    pub finished: u64,
    /// The numbers of the pictures in progress, by channel.
    ids: sdrkit_core::PictureIds<String>,
}

impl HritMosaics {
    /// Ends the picture of `channel` and returns its number.
    fn close(&mut self, channel: &String) -> u32 {
        let id = self.ids.of(channel);
        self.ids.close(channel);
        id
    }

    /// Takes the decompressed file `bytes` named `name`. Returns false when the name is not a segment name. A segment with
    /// bad headers counts in [`HritMosaics::bad`].
    pub fn push(&mut self, name: &str, bytes: &[u8], out: &mut impl FnMut(MosaicOut)) -> bool {
        let Some(seg) = segment_name(name) else {
            return false;
        };
        let Some((h, data_at)) = parse_headers(bytes, Dialect::Gk2a)
            .ok()
            .map(|h| (h.clone(), h.header_len as usize))
        else {
            self.bad += 1;
            return true;
        };
        let Some(img) = h.image.as_ref().filter(|i| {
            i.compression == Compression::None
                && (1..=22_000).contains(&i.columns)
                && (1..=22_000).contains(&i.lines)
                && (i.bits == 8 || i.bits == 16)
        }) else {
            self.bad += 1;
            return true;
        };
        let (w, l) = (u32::from(img.columns), u32::from(img.lines));
        let bpp = if img.bits == 16 { 2 } else { 1 };
        let need = (w * l) as usize * bpp;
        if bytes.len() < data_at.saturating_add(need) {
            self.bad += 1;
            return true;
        }
        let total = h.segment.map_or(10, |s| u32::from(s.total));
        let seq = h
            .segment
            .map_or(u32::from(seg.number), |s| u32::from(s.sequence));
        let pixels = u64::from(w) * u64::from(l) * u64::from(total);
        if !(1..=64).contains(&total) || !(1..=total).contains(&seq) || pixels > MAX_PICTURE {
            self.bad += 1;
            return true;
        }
        let differs = self.open.get(&seg.channel).is_some_and(|m| {
            m.stamp != seg.stamp || m.width != w || m.segment_lines != l || m.total != total
        });
        if differs {
            if let Some(old) = self.open.remove(&seg.channel) {
                let id = self.close(&seg.channel);
                if let Some(p) = old.picture(true, id, true) {
                    out(MosaicOut::Done {
                        picture: p,
                        complete: false,
                    });
                }
            }
        }
        let m = self
            .open
            .entry(seg.channel.clone())
            .or_insert_with(|| Mosaic {
                channel: seg.channel.clone(),
                stamp: seg.stamp.clone(),
                width: w,
                segment_lines: l,
                total,
                landed: vec![false; total as usize],
                pixels: vec![0; (w * l * total) as usize],
            });
        let data = &bytes[data_at..data_at + need];
        let at = ((seq - 1) * l * w) as usize;
        let dst = &mut m.pixels[at..at + (w * l) as usize];
        if bpp == 2 {
            for (d, s) in dst.iter_mut().zip(data.as_chunks::<2>().0) {
                let c = u16::from_be_bytes(*s);
                *d = if c >= 1023 { 0 } else { (c / 4) as u8 };
            }
        } else {
            dst.copy_from_slice(data);
        }
        m.landed[(seq - 1) as usize] = true;
        let received = m.received();
        self.rows.insert(
            seg.channel.clone(),
            (seg.stamp.clone(), w, l * total, received, total),
        );
        if received == total {
            if let Some(done) = self.open.remove(&seg.channel) {
                self.finished += 1;
                let id = self.close(&seg.channel);
                if let Some(p) = done.picture(false, id, true) {
                    out(MosaicOut::Done {
                        picture: p,
                        complete: true,
                    });
                }
            }
        } else if let Some(p) = m.picture(false, self.ids.of(&seg.channel), false) {
            out(MosaicOut::Progress(p));
        }
        true
    }

    /// Pushes out every open picture as partial (the end of the input).
    pub fn flush(&mut self, out: &mut impl FnMut(MosaicOut)) {
        for (channel, m) in std::mem::take(&mut self.open) {
            let id = self.close(&channel);
            if let Some(p) = m.picture(true, id, true) {
                out(MosaicOut::Done {
                    picture: p,
                    complete: false,
                });
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------------------------
// The file layer component.

/// A row of the table of recent files.
#[derive(Clone, Debug, PartialEq)]
pub struct FileRow {
    /// A running number (the row key).
    pub n: u64,
    /// The name.
    pub name: String,
    /// Bytes.
    pub size: usize,
    /// Parts.
    pub parts: u32,
    /// Parts received.
    pub received: u32,
    /// All parts came.
    pub complete: bool,
    /// Signal time at which the file was given out, s.
    pub time_s: f64,
}

/// Counters of [`FilecastComponent`].
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FilecastStats {
    /// Transport packets on the PID.
    pub pid_packets: u64,
    /// IP datagrams.
    pub datagrams: u64,
    /// Datagrams that are not IPv4 UDP.
    pub not_udp: u64,
    /// File-broadcast packets.
    pub packets: u64,
    /// File-broadcast packets refused.
    pub bad_packets: u64,
    /// Complete files.
    pub complete: u64,
    /// Incomplete files.
    pub incomplete: u64,
    /// Files whose bzip2 data failed.
    pub failed: u64,
    /// Files dropped because the worker's queue was full.
    pub dropped: u64,
    /// Bytes of files.
    pub bytes: u64,
    /// Files open.
    pub open: usize,
    /// The last error.
    pub last_error: String,
    /// Pictures finished.
    pub pictures: u64,
}

/// The report of [`FilecastComponent`].
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FilecastReport {
    /// The counters.
    pub stats: FilecastStats,
    /// The PID read.
    pub pid: u16,
    /// The newest files, newest last.
    pub recent: Vec<FileRow>,
    /// The pictures: channel, stamp, width, height, segments received, total.
    pub pictures: Vec<(String, String, u32, u32, u32, u32)>,
    /// Log lines since the last report.
    pub log: Vec<String>,
}

impl Item for FilecastReport {
    const KIND: &'static str = "satimg.filecast_report";
}

/// The PID of the datagram sections.
pub const PID: Setting<field::I64> = Setting::new("pid", "PID of the MPE stream", "", 256)
    .range(32.0, 8190.0, 1.0)
    .help("The transport stream PID that carries the datagram sections (256 in the test signal).");
/// The part size of the file broadcast.
pub const PART_BYTES: Setting<field::I64> = Setting::new("part_bytes", "Part size", "B", 1408)
    .range(1.0, 8192.0, 1.0)
    .help("Bytes of each numbered part of a file (1408 in the test signal). Not confirmed by a public document.");
/// Characters of the tag at the end of a GEONETCAST name that the stack removes (not confirmed by a public document).
pub const NAME_TAG: Setting<field::I64> =
    Setting::new("name_tag", "Name tag of GEONETCAST", "characters", 45)
        .range(0.0, 200.0, 1.0)
        .help("GEONETCAST names end with a tag of this many characters, which the stack removes.")
        .advanced();
/// Seconds of silence after which a file is given up.
pub const MAX_SILENCE: Setting<field::F64> =
    Setting::new("max_silence", "Give up on a file after", "s", 120.0).range(10.0, 3600.0, 1.0);

sdrkit_core::settings! {
    /// The settings of [`FilecastComponent`].
    pub enum FilecastComponentSetting {
        /// The PID.
        Pid = PID,
        /// The part size.
        PartBytes = PART_BYTES,
        /// The silence after which a file is given up.
        MaxSilence = MAX_SILENCE,
        /// The name tag of GEONETCAST.
        NameTag = NAME_TAG,
    }
}

/// The `clear` command of the file broadcasts.
pub const CLEAR: Command = Command::new("clear", "Clear")
    .help("Forgets the open files and the counters; keeps the pictures.");

sdrkit_core::commands! {
    /// The commands of [`FilecastComponent`].
    pub enum FilecastCommand {
        /// Forgets the open files and the counters.
        Clear = CLEAR,
    }
}

/// The result of a bzip2 job.
type Unpacked = (
    FinishedFile,
    f64,
    Result<Vec<u8>, sdrkit_codec_bzip2::Error>,
);

/// Transport packets in; files, pictures and the report out. Decompression runs on the helper pool when the input is live, in
/// line otherwise.
pub struct FilecastComponent {
    service: Service,
    tag: usize,
    pid: u16,
    cfg: FilecastConfig,
    mpe: MpeReader,
    asm: FileAssembler,
    mosaics: HritMosaics,
    stats: FilecastStats,
    recent: VecDeque<FileRow>,
    n: u64,
    log: Vec<String>,
    pool: Option<Pool>,
    live: bool,
    jobs: Vec<(usize, Ticket<Unpacked>)>,
    queued: usize,
    files: Vec<Timed<SatFile>>,
    pictures: Vec<Timed<Picture>>,
    now: f64,
}

impl FilecastComponent {
    /// The file layer of `service`.
    pub fn new(service: Service) -> Self {
        let cfg = FilecastConfig::default();
        FilecastComponent {
            service,
            tag: GEONETCAST_TAG,
            pid: 256,
            cfg,
            mpe: MpeReader::new(256),
            asm: FileAssembler::new(cfg),
            mosaics: HritMosaics::default(),
            stats: FilecastStats::default(),
            recent: VecDeque::new(),
            n: 0,
            log: Vec::new(),
            pool: None,
            live: false,
            jobs: Vec::new(),
            queued: 0,
            files: Vec::new(),
            pictures: Vec::new(),
            now: 0.0,
        }
    }

    fn restart(&mut self) {
        self.mpe = MpeReader::new(self.pid);
        self.asm = FileAssembler::new(self.cfg);
    }

    /// One IP datagram.
    fn datagram(&mut self, d: &[u8]) {
        self.stats.datagrams += 1;
        let ihl = usize::from(d[0] & 0x0F) * 4;
        if d[0] >> 4 != 4 || d.len() < 20 || d[9] != 17 || ihl < 20 || d.len() < ihl + 8 {
            self.stats.not_udp += 1;
            return;
        }
        let mut done = Vec::new();
        self.asm
            .push(&d[ihl + 8..], self.now, &mut |f| done.push(f));
        for f in done {
            self.file(f);
        }
    }

    /// One finished file of the broadcast.
    fn file(&mut self, f: FinishedFile) {
        let now = self.now;
        if !f.complete {
            self.stats.incomplete += 1;
            self.emit(f, FileKind::Incomplete, now);
            return;
        }
        if self.service == Service::Himawaricast && f.name.ends_with(".bz2") {
            let n = f.data.len();
            if self.live {
                if let Some(pool) = self.pool.as_ref() {
                    if self.queued + n > MAX_QUEUE {
                        self.stats.dropped += 1;
                        return;
                    }
                    self.queued += n;
                    let t = pool.spawn(move || {
                        let r = sdrkit_codec_bzip2::decompress(&f.data, MAX_BZIP2_OUT);
                        (f, now, r)
                    });
                    self.jobs.push((n, t));
                    return;
                }
            }
            let r = sdrkit_codec_bzip2::decompress(&f.data, MAX_BZIP2_OUT);
            self.unpacked(f, now, r);
            return;
        }
        let kind = match self.service {
            Service::Himawaricast => FileKind::Additional,
            Service::Geonetcast => FileKind::Complete,
        };
        self.stats.complete += 1;
        self.emit(f, kind, now);
    }

    /// A bzip2 file after its decompression.
    fn unpacked(
        &mut self,
        f: FinishedFile,
        time: f64,
        r: Result<Vec<u8>, sdrkit_codec_bzip2::Error>,
    ) {
        match r {
            Ok(data) => {
                self.stats.complete += 1;
                let name = f.name.strip_suffix(".bz2").unwrap_or(&f.name).to_owned();
                let mut pics = Vec::new();
                let segment = self
                    .mosaics
                    .push(&clean_name(&name), &data, &mut |o| pics.push(o));
                for o in pics {
                    match o {
                        MosaicOut::Progress(p) => self.pictures.push(Timed { time, value: p }),
                        MosaicOut::Done { picture, complete } => {
                            if complete {
                                self.stats.pictures += 1;
                            }
                            self.pictures.push(Timed {
                                time,
                                value: picture,
                            });
                        }
                    }
                }
                let (kind, name) = if segment {
                    let n = if name.contains('.') {
                        name
                    } else {
                        format!("{name}.hrit")
                    };
                    (FileKind::HritSegment, n)
                } else {
                    (FileKind::Additional, name)
                };
                self.emit(FinishedFile { name, data, ..f }, kind, time);
            }
            Err(e) => {
                self.stats.failed += 1;
                let line = format!("damaged bzip2 data in {}", clean_name(&f.name));
                self.stats.last_error = format!("{line}: {e}");
                self.log.push(line);
                self.emit(f, FileKind::BadBzip2, time);
            }
        }
    }

    /// Gives a file out with its cleaned name (a name that cleans to nothing is not given out).
    fn emit(&mut self, f: FinishedFile, kind: FileKind, time: f64) {
        let mut raw = f.name;
        if self.service == Service::Geonetcast && raw.chars().count() > self.tag {
            let keep = raw.chars().count() - self.tag;
            raw = raw.chars().take(keep).collect();
        }
        let name = clean_name(&raw);
        self.stats.bytes += f.data.len() as u64;
        if name.is_empty() {
            return;
        }
        self.n += 1;
        self.recent.push_back(FileRow {
            n: self.n,
            name: name.clone(),
            size: f.data.len(),
            parts: f.parts,
            received: f.received,
            complete: f.complete,
            time_s: time,
        });
        while self.recent.len() > RECENT {
            self.recent.pop_front();
        }
        self.files.push(Timed {
            time,
            value: SatFile {
                name,
                kind,
                bytes: Arc::from(f.data),
                parts: f.parts,
                received: f.received,
                complete: f.complete,
                time,
            },
        });
    }

    /// Collects finished jobs; with `wait`, waits for all.
    fn collect(&mut self, wait: bool) {
        let jobs = std::mem::take(&mut self.jobs);
        for (n, mut t) in jobs {
            let r = if wait {
                Some(t.wait())
            } else {
                match t.try_take() {
                    Some(r) => Some(r),
                    None => {
                        self.jobs.push((n, t));
                        continue;
                    }
                }
            };
            self.queued -= n;
            if let Some(Ok((f, time, r))) = r {
                self.unpacked(f, time, r);
            } else {
                self.stats.failed += 1;
            }
        }
    }

    fn write(&mut self, io: &mut Io, report: bool) {
        io.items_out::<SatFile>(0).append(&mut self.files);
        io.items_out::<Picture>(1).append(&mut self.pictures);
        if report {
            self.stats.pid_packets = self.mpe.stats.packets;
            self.stats.packets = self.asm.stats.packets;
            self.stats.bad_packets = self.asm.stats.bad;
            self.stats.open = self.asm.open_files();
            let r = FilecastReport {
                stats: self.stats.clone(),
                pid: self.pid,
                recent: self.recent.iter().cloned().collect(),
                pictures: self
                    .mosaics
                    .rows
                    .iter()
                    .map(|(c, (s, w, h, r, t))| (c.clone(), s.clone(), *w, *h, *r, *t))
                    .collect(),
                log: std::mem::take(&mut self.log),
            };
            io.items_out::<FilecastReport>(2).push(Timed {
                time: self.now,
                value: r,
            });
        }
    }

    fn take(&mut self, io: &mut Io) {
        let input = io.input(0);
        let (first, tags) = (input.first(), input.tags().to_vec());
        let packets = io.items::<TsPacket>(0);
        for (k, p) in packets.iter().enumerate() {
            let lost = tags.iter().any(|t| {
                t.at == first + k as u64 && matches!(t.kind, TagKind::Discontinuity { .. })
            });
            if lost {
                self.mpe.loss();
            }
            self.now = self.now.max(p.time);
            let mut dgrams: Vec<Vec<u8>> = Vec::new();
            self.mpe.push(p.value.bytes(), |d| dgrams.push(d.to_vec()));
            for d in dgrams {
                self.datagram(&d);
            }
        }
        let mut done = Vec::new();
        let now = self.now;
        self.asm.expire(now, &mut |f| done.push(f));
        for f in done {
            self.file(f);
        }
    }
}

impl Component for FilecastComponent {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "satimg.filecast",
            vec![sdrkit_proto_mpegts::stream::port("in")],
            vec![
                Port::typed::<SatFile>("files").rate_from(RateFrom::Fixed(0.0)),
                Port::typed::<Picture>("pictures").rate_from(RateFrom::Fixed(0.0)),
                Port::typed::<FilecastReport>("report").rate_from(RateFrom::Fixed(0.0)),
            ],
        )
        .with_settings(FilecastComponentSetting::schema())
    }
    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.restart();
        Ok(vec![
            Format::new(Kind::typed::<SatFile>(), 0.0),
            Format::new(Kind::typed::<Picture>(), 0.0),
            Format::new(Kind::typed::<FilecastReport>(), 0.0),
        ])
    }
    fn attach(&mut self, ctx: &RunContext) {
        self.live = ctx.live;
        self.pool = Some(ctx.pool.clone());
    }
    fn process(&mut self, io: &mut Io) {
        self.take(io);
        self.collect(false);
        let tick = io.tick();
        self.write(io, tick);
    }
    fn finish(&mut self, io: &mut Io) {
        self.take(io);
        let mut done = Vec::new();
        let now = self.now;
        self.asm.flush(now, &mut |f| done.push(f));
        for f in done {
            self.file(f);
        }
        self.collect(true);
        let mut pics = Vec::new();
        self.mosaics.flush(&mut |o| pics.push(o));
        for o in pics {
            if let MosaicOut::Done { picture, .. } = o {
                self.pictures.push(Timed {
                    time: now,
                    value: picture,
                });
            }
        }
        self.write(io, true);
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match FilecastComponentSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            FilecastComponentSetting::Pid => self.pid = PID.read(id, v)? as u16,
            FilecastComponentSetting::PartBytes => {
                self.cfg.part_bytes = PART_BYTES.read(id, v)? as usize
            }
            FilecastComponentSetting::MaxSilence => {
                self.cfg.max_silence_s = MAX_SILENCE.read(id, v)?
            }
            FilecastComponentSetting::NameTag => self.tag = NAME_TAG.read(id, v)? as usize,
        }
        self.restart();
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([
            (
                FilecastComponentSetting::Pid,
                Value::I64(i64::from(self.pid)),
            ),
            (
                FilecastComponentSetting::PartBytes,
                Value::I64(self.cfg.part_bytes as i64),
            ),
            (
                FilecastComponentSetting::MaxSilence,
                Value::F64(self.cfg.max_silence_s),
            ),
            (
                FilecastComponentSetting::NameTag,
                Value::I64(self.tag as i64),
            ),
        ])
    }
    fn commands(&self) -> &Commands {
        FilecastCommand::commands()
    }
    fn invoke(&mut self, id: CommandId, _: &Values) -> Result<Option<Record>, CommandError> {
        match FilecastCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            FilecastCommand::Clear => {
                self.restart();
                let pictures = self.stats.pictures;
                self.stats = FilecastStats {
                    pictures,
                    ..Default::default()
                };
                self.recent.clear();
            }
        }
        Ok(None)
    }
}

// ---------------------------------------------------------------------------------------------------------------------------------
// Settings of the stack.

/// The symbol rate (0: found from the spectrum). The services' rates are data that the host copies in.
pub const SYMBOL_RATE: Setting<field::Hz> = s2::symbol_rate(0.0);
/// The roll-off.
pub const ROLL_OFF: Setting<field::Choice<RollOffChoice>> = s2::roll_off(RollOffChoice::Auto);
/// The symbol rate of the test signal (0: the setting `symbol_rate`, else 2 586 148).
pub const GEN_SYMBOL_RATE: Setting<field::Hz> =
    Setting::new("gen_symbol_rate", "Symbol rate", "Hz", 0.0)
        .range(0.0, 60e6, 0.0)
        .zero_means("The setting symbol_rate, else 2 586 148")
        .section(TEST_SIGNAL)
        .advanced();

sdrkit_core::settings! {
    /// The settings of `sat_filecast`.
    pub enum FilecastSetting {
        /// The symbol rate.
        SymbolRate = SYMBOL_RATE,
        /// The roll-off.
        RollOff = ROLL_OFF,
        /// The carrier.
        CarrierOffset = CARRIER_OFFSET,
        /// The PID.
        Pid = PID,
        /// The part size.
        PartBytes = PART_BYTES,
        /// The silence after which a file is given up.
        MaxSilence = MAX_SILENCE,
        /// The name tag of GEONETCAST.
        NameTag = NAME_TAG,
        /// The input stream.
        Isi = ISI,
        /// Which MODCODs are decoded.
        ModcodRule = MODCOD_RULE,
        /// The MODCOD of the rule "fixed".
        Modcod = MODCOD,
        /// The MODCODs of the rule "set".
        Modcods = MODCODS,
        /// Which frame length is decoded.
        FrameLength = FRAME_LENGTH,
        /// The expected pilots.
        Pilots = PILOTS_RULE,
        /// The PL scrambling code.
        PlScrambling = PL_SCRAMBLING,
        /// The symbol rate of the test signal.
        GenSymbolRate = GEN_SYMBOL_RATE,
    }
}

sdrkit_core::settings! {
    /// The settings of [`FilecastDisplay`].
    pub enum FilecastDisplaySetting {
        /// The symbol rate.
        SymbolRate = SYMBOL_RATE,
    }
}

// ---------------------------------------------------------------------------------------------------------------------------------
// The display.

fn display_declaration() -> ResultsDecl {
    ResultsDecl::new()
        .with_fields(FilecastFields::SHAPE.fields)
        .with_table(RECENT_FILES.def())
        .with_table(FILECAST_PICTURES.def())
}

/// What `sat_filecast` publishes.
pub fn declaration() -> ResultsDecl {
    crate::typed::shape(ID).results_decl()
}

const NOTE_TEXT: &str = "Experimental: not confirmed on a real signal. No public document was read for the file broadcast. \
    Not confirmed by a public document: the packet layout of the file broadcast (types, lengths, ids, part numbers, the name \
    and the size in the head), the part sizes, the PIDs, the 45-character tag at the end of GEONETCAST names and the \
    service symbol rates. The MPE layout follows EN 301 192 as written from knowledge of it. IPv6 datagrams are not read.";

/// The display of `sat_filecast`.
pub struct FilecastDisplay {
    service: Service,
    published: Published,
    symbol_rate: f64,
    summary: String,
    hint: String,
    key: String,
}

impl FilecastDisplay {
    /// The display of `service`.
    pub fn new(service: Service) -> Self {
        FilecastDisplay {
            service,
            published: super::display::published(display_declaration()),
            symbol_rate: 0.0,
            summary: String::new(),
            hint: String::new(),
            key: String::new(),
        }
    }
}

impl Component for FilecastDisplay {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::sink(
            "satimg.filecast_display",
            vec![
                Port::typed::<LinkState>("link"),
                Port::typed::<FilecastReport>("files"),
            ],
        )
        .with_settings(FilecastDisplaySetting::schema())
    }
    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        Ok(vec![])
    }
    fn process(&mut self, io: &mut Io) {
        for r in io.items::<FilecastReport>(1) {
            super::display::log_lines(self.published.results_mut(), &r.value.log);
        }
        if !io.tick() {
            return;
        }
        let link = link_of(io, 0).unwrap_or_default();
        let r = last::<FilecastReport>(io, 1).cloned().unwrap_or_default();
        let st = &r.stats;
        let p = &mut self.published;
        for f in &r.recent {
            self.key.clear();
            let _ = write!(self.key, "{}", f.n);
            let row = RecentFileRow {
                name: &f.name,
                size: f.size as i64,
                parts: i64::from(f.parts),
                received: i64::from(f.received),
                complete: f.complete,
                time_s: f.time_s,
            };
            RECENT_FILES.put(p, &self.key, &row);
        }
        for (c, s, w, h, n, t) in &r.pictures {
            let row = FilecastPictureRow {
                channel: c,
                stamp: s,
                width: i64::from(*w),
                height: i64::from(*h),
                segments: i64::from(*n),
                total: i64::from(*t),
            };
            FILECAST_PICTURES.put(p, c, &row);
        }
        let name = self.service.label();
        let files = st.complete + st.incomplete + st.failed;
        let state = if files > 0 && link.state >= 2 {
            3
        } else {
            link.state.min(2)
        };
        s2::hint(&link, self.symbol_rate, "", &mut self.hint);
        p.put(&FilecastFields {
            pid: i64::from(r.pid),
            pid_packets: st.pid_packets as i64,
            datagrams: st.datagrams as i64,
            not_udp: st.not_udp as i64,
            broadcast_packets: st.packets as i64,
            bad_packets: st.bad_packets as i64,
            files_complete: st.complete as i64,
            files_incomplete: st.incomplete as i64,
            files_failed: st.failed as i64,
            files_dropped: st.dropped as i64,
            bytes: st.bytes as i64,
            open_files: st.open as i64,
            last_error: &st.last_error,
            pictures_finished: st.pictures as i64,
            hint: &self.hint,
            note: NOTE_TEXT,
        });
        self.summary.clear();
        let _ = if !self.hint.is_empty() {
            write!(self.summary, "{name}, {}", self.hint)
        } else if files > 0 {
            write!(
                self.summary,
                "{name}, {files} files, {} pictures",
                st.pictures
            )
        } else if link.state >= 1 {
            write!(self.summary, "{name}, carrier found, no files yet")
        } else {
            write!(self.summary, "{name}, searching")
        };
        s2::write_status(p, &link, state, &self.summary);
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match FilecastDisplaySetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            FilecastDisplaySetting::SymbolRate => self.symbol_rate = SYMBOL_RATE.read(id, v)?,
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([(
            FilecastDisplaySetting::SymbolRate,
            Value::Hz(self.symbol_rate),
        )])
    }
    /// The files and the pictures stay where `kept` holds them.
    fn forget(&mut self, kept: &sdrkit_graph::Kept) {
        if !kept.table(RECENT_FILES.key) {
            self.published.clear_rows(RECENT_FILES.def());
        }
        if !kept.table(FILECAST_PICTURES.key) {
            self.published.clear_rows(FILECAST_PICTURES.def());
        }
    }
    fn results(&self) -> Option<&Results> {
        Some(self.published.results())
    }
}

// ---------------------------------------------------------------------------------------------------------------------------------
// The graph and the descriptor.

const PARTS: Dvbs2Parts = Dvbs2Parts {
    ts: true,
    field: false,
};

/// The graph of `service`.
pub fn builder(service: Service) -> GraphBuilder {
    attach(GraphBuilder::new(), PARTS)
        .add("filecast", FilecastComponent::new(service))
        .add("display", FilecastDisplay::new(service))
        .connect("s2_bbframe.ts", "filecast.in")
        .connect("s2_report.link", "display.link")
        .connect("filecast.report", "display.files")
        .output("filecast.files")
        .output("filecast.pictures")
        .status_from("display")
        .clock(REPORT_S)
}

/// The receiver of the submode of `cfg`.
///
/// # Errors
///
/// The errors of [`GraphBuilder::build`] and of [`sdrkit_graph::StackGraphBuilder::build`], among them a refused setting.
pub fn receiver(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    let service = Service::of(cfg.submode.as_deref());
    let mut g = builder(service)
        .catalog(&Catalog::of(cfg)?)
        .declare(&declaration())
        .build()?;
    g.set_max_items(1 << 20);
    let mut b = StackGraph::of(crate::typed::shape(ID), g)
        .min_rate(1e6, "File broadcasts need more samples")
        .command(
            "clear",
            sdrkit_graph::CommandTarget::of("filecast", "clear"),
        );
    for (key, targets) in routes(PARTS) {
        let t: Vec<&str> = match key {
            "symbol_rate" => targets
                .iter()
                .copied()
                .chain(["display.symbol_rate"])
                .collect(),
            _ => targets.to_vec(),
        };
        b = b.route(key, &t)?;
    }
    b.route("pid", &["filecast.pid"])?
        .route("part_bytes", &["filecast.part_bytes"])?
        .route("max_silence", &["filecast.max_silence"])?
        .route("name_tag", &["filecast.name_tag"])?
        .with_settings(&cfg.settings)
        .build()
}

fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    super::stack_receiver(receiver(cfg))
}

fn make_generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(Box::new(generator(cfg)?))
}

fn schema() -> Arc<sdrkit_core::Schema> {
    FilecastSetting::schema()
}

/// File broadcasts on DVB-S2.
pub static DESCRIPTOR: StackDescriptor = StackDescriptor {
    id: ID,
    name: "Satellite file broadcasts",
    group: crate::GROUP,
    blurb: "HimawariCast and GEONETCAST Americas on DVB-S2 (C band, through the LNB): files and HimawariCast full-disc pictures.",
    // The stack holds no frequency: the host sets it from the downlinks of data/links/xband_downlinks.csv (the frequency
    // after the LNB). Without one the receiver follows the centre of the input.
    tuning: Tuning {
        min_hz: 950e6,
        max_hz: 2150e6,
        ..Tuning::anywhere(0.0, ASKED_RATE, 4.45e6, 3.2e6)
    },
    submodes: &[
        SubMode {
            id: HIMAWARICAST,
            name: "HimawariCast",
            default_hz: 0.0,
        },
        SubMode {
            id: GEONETCAST,
            name: "GEONETCAST Americas",
            default_hz: 0.0,
        },
    ],
    settings: schema,
    make_receiver,
    make_generator: Some(make_generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[],
    auto_gain: AutoGain::Radio,
    verify: Some(&crate::verify::FILECAST_HOOKS),
    benchmarks: None,
};

// ---------------------------------------------------------------------------------------------------------------------------------
// The test signal.

/// The source address of the test datagrams.
pub const TEST_SRC: Ipv4Addr = Ipv4Addr::new(10, 0, 0, 1);
/// The destination address of the test datagrams.
pub const TEST_DST: Ipv4Addr = Ipv4Addr::new(239, 0, 0, 1);
/// The MAC address of the test sections.
pub const TEST_MAC: [u8; 6] = [0x01, 0x00, 0x5E, 0x00, 0x00, 0x01];
/// Columns of a test segment.
pub const TEST_COLUMNS: u32 = 640;
/// Lines of a test segment.
pub const TEST_LINES: u32 = 64;
/// Segments of a test disc.
pub const TEST_SEGMENTS: u32 = 10;

/// The 10-bit count of pixel (x, y) of the test disc: ((3 x + 5 y) mod 1000), plus 11 when y / 16 is odd.
pub fn test_count(x: u32, y: u32) -> u16 {
    (((3 * x + 5 * y) % 1000) + if (y / 16) % 2 == 1 { 11 } else { 0 }) as u16
}

/// The time stamp of test round `round`: 2026-11-09 02:30 plus 10 minutes a round.
pub fn test_stamp(round: u32) -> String {
    let m = 2 * 60 + 30 + 10 * round;
    let (d, m) = (9 + m / 1440, m % 1440);
    format!("202611{d:02}{:02}{:02}", m / 60, m % 60)
}

/// HRIT segment `n` (1 to 10) of the test disc of time stamp `stamp`, not compressed, named `name`.
pub fn test_segment(name: &str, n: u32) -> Vec<u8> {
    let mut rec = vec![n as u8, TEST_SEGMENTS as u8];
    rec.extend_from_slice(&((TEST_LINES * (n - 1) + 1) as u16).to_be_bytes());
    let b = Builder::new(0)
        .image(16, TEST_COLUMNS as u16, TEST_LINES as u16, 0)
        .annotation(name)
        .record(128, &rec);
    let mut f = b.build((TEST_COLUMNS * TEST_LINES * 2) as usize);
    for y in (n - 1) * TEST_LINES..n * TEST_LINES {
        for x in 0..TEST_COLUMNS {
            f.extend_from_slice(&test_count(x, y).to_be_bytes());
        }
    }
    f
}

/// The files of test round `round` of `service`, as (name, bytes).
pub fn test_files(service: Service, round: u32) -> Vec<(String, Vec<u8>)> {
    match service {
        Service::Himawaricast => {
            let stamp = test_stamp(round);
            let mut out: Vec<(String, Vec<u8>)> = (1..=TEST_SEGMENTS)
                .map(|n| {
                    let name = format!("IMG_DK01IR1_{stamp}_{n:03}");
                    let seg = test_segment(&name, n);
                    (format!("{name}.bz2"), sdrkit_codec_bzip2::compress(&seg, 9))
                })
                .collect();
            out.push((
                "NOTICE_202611090230.txt.bz2".into(),
                sdrkit_codec_bzip2::compress(b"sdrkit test notice\n", 9),
            ));
            out
        }
        Service::Geonetcast => (0..11u32)
            .map(|n| {
                let name = format!(
                    "OR_TEST-{n:02}_G16_s2026313.nc{}",
                    "X".repeat(GEONETCAST_TAG)
                );
                let data = (0..5000 + 700 * n as usize)
                    .map(|i| ((7 * i + n as usize) % 256) as u8)
                    .collect();
                (name, data)
            })
            .collect(),
    }
}

/// The datagrams (UDP payloads) of one file of the carousel: a head of 242 bytes, the parts, the head again, a tail.
pub fn carousel_packets(id: u32, name: &str, data: &[u8], part: usize) -> Vec<Vec<u8>> {
    let parts = data.len().div_ceil(part.max(1)).max(1);
    let head = head_packet(id, name, data.len() as u32, parts as u16, 242);
    let mut out = vec![head.clone()];
    for (k, c) in data.chunks(part.max(1)).enumerate() {
        out.push(part_packet(id, k as u16, c));
    }
    out.push(head);
    out.push(tail_packet(id));
    out
}

/// Null packets before the first round of the test carousel, so that a receiver locks before the first file starts.
pub const TEST_LEAD: usize = 2048;
/// The fewest transport packets of a round of the test carousel; null packets fill the rest.
pub const TEST_ROUND: usize = 2048;

/// The transport stream of the test carousel: each datagram as UDP from 10.0.0.1:5000 to 239.0.0.1:8001 in one datagram
/// section on PID `pid`, rounds of [`test_files`] after each other, behind [`TEST_LEAD`] null packets and each round filled
/// with null packets to [`TEST_ROUND`].
pub struct CarouselSource {
    service: Service,
    pid: u16,
    part: usize,
    round: u32,
    id: u32,
    cc: u8,
    queue: VecDeque<[u8; 188]>,
}

impl CarouselSource {
    /// The carousel of `service` on `pid` with parts of `part` bytes.
    pub fn new(service: Service, pid: u16, part: usize) -> Self {
        CarouselSource {
            service,
            pid,
            part,
            round: 0,
            id: 0x1000,
            cc: 0,
            queue: std::iter::repeat_n(sdrkit_proto_mpegts::source::null_packet(), TEST_LEAD)
                .collect(),
        }
    }

    fn round(&mut self) {
        let mut ip = Vec::new();
        let mut sec = Vec::new();
        for (name, data) in test_files(self.service, self.round) {
            for d in carousel_packets(self.id, &name, &data, self.part) {
                ip.clear();
                sec.clear();
                let ok = write_ipv4_udp_plain(&mut ip, TEST_SRC, TEST_DST, 5000, 8001, &d).is_ok()
                    && write_section(TEST_MAC, &ip, &mut sec).is_ok();
                if ok {
                    let q = &mut self.queue;
                    section_packets(&sec, self.pid, &mut self.cc, |p| q.push_back(p));
                }
            }
            self.id += 1;
        }
        while self.queue.len() < TEST_ROUND {
            self.queue
                .push_back(sdrkit_proto_mpegts::source::null_packet());
        }
        self.round += 1;
    }
}

impl PacketSource for CarouselSource {
    fn next_packet(&mut self, out: &mut [u8; 188]) {
        if self.queue.is_empty() {
            self.round();
        }
        *out = self
            .queue
            .pop_front()
            .unwrap_or_else(sdrkit_proto_mpegts::source::null_packet);
    }
}

/// The test signal of the submode of `cfg`: DVB-S2 with roll-off 0.20, normal frames, pilots, MODCOD 6 (HimawariCast) or 15
/// (GEONETCAST), carrying [`CarouselSource`] on the PID and with the part size of the settings.
///
/// # Errors
///
/// [`ConfigError`] if the symbol rate does not fit the sample rate.
pub fn generator(cfg: &GenConfig) -> Result<s2::S2Transmitter, ConfigError> {
    let service = Service::of(cfg.submode.as_deref());
    let s = &cfg.settings;
    let rs = [s.get(&GEN_SYMBOL_RATE), s.get(&SYMBOL_RATE)]
        .into_iter()
        .find(|&r| r > 0.0)
        .unwrap_or(TEST_SYMBOL_RATE);
    let pid = s.get(&PID) as u16;
    let part = s.get(&PART_BYTES) as usize;
    s2::transmitter(
        cfg.rate_hz,
        rs,
        0.20,
        service.test_modcod(),
        true,
        Box::new(CarouselSource::new(service, pid, part)),
        None,
    )
}
