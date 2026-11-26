//! GOES-R Rebroadcast (`goes_grb`): 2048-byte CADUs in the data fields of a DVB-S2 generic stream, AOS frames of virtual
//! channels 5 and 6, space packets with an 8-byte secondary header and a CRC-32, payloads of one or more packets, ABI and
//! SUVI picture blocks in JPEG 2000, GLM flashes, groups and events (module [`super::glm`]), and the other generic
//! payloads as files.
//!
//! The GOES-R Series Product Definition and Users' Guide, Volume 4: GRB (revision H.1) confirms the virtual channels 5
//! and 6 (4.4.2.1), the CRC (4.5.3), the 21-byte generic payload header (5.3.1), the GLM APIDs (A.2) and the GLM records
//! (7.2.1.6). Its secondary header (4.5.2) is not consistent: the table gives the payload variant 5 bits and the assembler
//! 2, the text gives the variant bits 101 to 103 and the assembler bits 104 to 107. The stack reads 5 bits; variant 0 reads
//! the same in both. The symbol rate, the picture block header, the other APIDs and the reduction of ABI pictures are not
//! checked against it.
//!
//! Standard: ETSI EN 302 307-1 (DVB-S2); CCSDS 131.0-B (marker), 732.0-B (AOS), 133.0-B (space packets); ITU-T T.800
//! (JPEG 2000).

use super::glm::{self, Glm, GlmReport, Taken};
use super::s2::{
    self, attach, cadu_state, crc_packet, frame_fields, link_of, routes, CaduStream, Dvbs2Parts,
    FieldFrames, FieldFramesComponent, FieldFramesReport, LinkState, CARRIER_OFFSET, FRAME_LENGTH,
    MODCOD, MODCODS, MODCOD_RULE, PILOTS_RULE, PL_SCRAMBLING, ZONE_BYTES,
};
use crate::components::last;
use crate::gen::ZoneStream;
use crate::items::{FileKind, SatFile, WholePacket};
use crate::typed::{
    CaduFields, GlmFields, GlmFlashRow, GlmGroupRow, GrbApidRow, GrbFields, GrbProductRow,
    UhritFrameRow, GLM_FLASHES, GLM_GROUPS, GRB_PACKETS_BY_APID, GRB_PRODUCTS, UHRIT_FRAMES,
};
use crate::REPORT_S;
use sdrkit_codec_image::j2k;
use sdrkit_codec_image::strip::Strip;
use sdrkit_core::command::{Command, CommandError, CommandId, CommandSet, Commands};
use sdrkit_core::interface::{Fields as _, Seconds, Time};
use sdrkit_core::publish::{field, Published};
use sdrkit_core::{
    AutoGain, ConfigError, GenConfig, Generator, LockProfile, Receiver, Record, Results,
    ResultsDecl, Setting, SettingError, SettingId, SettingSet, StackConfig, StackDescriptor,
    Tuning, Value, Values,
};
use sdrkit_graph::{
    no_values, values_of, Applied, Catalog, Component, ComponentInfo, Format, GraphBuilder,
    GraphError, Io, Item, Kind, Port, RateFrom, StackGraph, Timed,
};
use sdrkit_proto_dvbs2::settings::RollOffChoice;
use sdrkit_util_bits::crc::catalog::CRC32_ISO_HDLC;
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::sync::Arc;

/// The stack id.
pub const ID: &str = "goes_grb";
/// The symbol rate of the link (not confirmed by a public document).
pub const LINK_SYMBOL_RATE: f64 = 8_665_938.0;
/// The input rate the stack asks for.
pub const ASKED_RATE: f64 = 20e6;
/// The shortest packet read.
pub const MIN_PACKET: usize = 18;
/// Bytes of the picture block header.
pub const BLOCK_HEADER: usize = 34;
/// Bytes of the generic data header.
pub const GENERIC_HEADER: usize = 21;
/// The widest and tallest picture kept (full size); a block beyond it counts as other.
pub const MAX_SIDE: u32 = 22_000;
/// The most pixels of a picture that is not ABI (kept at full size).
pub const MAX_PIXELS: u64 = 1 << 26;

// ---------------------------------------------------------------------------------------------------------------------------------
// Products.

/// What an APID carries (the table is not confirmed by a public document).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Product {
    /// ABI full disc of a channel (1 to 16).
    AbiFullDisc(u8),
    /// ABI CONUS of a channel.
    AbiConus(u8),
    /// ABI mesoscale 1 of a channel.
    AbiMeso1(u8),
    /// ABI mesoscale 2 of a channel.
    AbiMeso2(u8),
    /// ABI metadata.
    AbiMetadata,
    /// GLM metadata.
    GlmMetadata,
    /// GLM events, flashes or groups (1, 2, 3).
    Glm(u8),
    /// SUVI metadata.
    SuviMetadata,
    /// A SUVI band (0 to 5: Fe094, Fe132, Fe171, Fe195, Fe284, Fe304).
    Suvi(u8),
    /// GRB information.
    Information,
    /// Not in the table.
    Unknown,
}

const SUVI: [&str; 6] = ["Fe094", "Fe132", "Fe171", "Fe195", "Fe284", "Fe304"];

impl Product {
    /// The product of `apid`.
    pub fn of(apid: u16) -> Product {
        let ch = |base: u16| (apid - base + 1) as u8;
        match apid {
            0x090..=0x09F => Product::AbiFullDisc(ch(0x090)),
            0x110..=0x11F => Product::AbiFullDisc(ch(0x110)),
            0x190..=0x19F => Product::AbiFullDisc(ch(0x190)),
            0x0B0..=0x0BF => Product::AbiConus(ch(0x0B0)),
            0x130..=0x13F => Product::AbiConus(ch(0x130)),
            0x1A0..=0x1AF => Product::AbiConus(ch(0x1A0)),
            0x0D0..=0x0DF => Product::AbiMeso1(ch(0x0D0)),
            0x150..=0x15F => Product::AbiMeso1(ch(0x150)),
            0x0F0..=0x0FF => Product::AbiMeso2(ch(0x0F0)),
            0x170..=0x17F => Product::AbiMeso2(ch(0x170)),
            0x080..=0x19F => Product::AbiMetadata,
            0x300 => Product::GlmMetadata,
            0x301..=0x303 => Product::Glm((apid - 0x300) as u8),
            0x480..=0x485 => Product::SuviMetadata,
            0x486..=0x48B => Product::Suvi((apid - 0x486) as u8),
            0x580 => Product::Information,
            _ => Product::Unknown,
        }
    }

    /// Its name ("ABI CONUS C13").
    pub fn name(self) -> String {
        match self {
            Product::AbiFullDisc(c) => format!("ABI full disc C{c:02}"),
            Product::AbiConus(c) => format!("ABI CONUS C{c:02}"),
            Product::AbiMeso1(c) => format!("ABI mesoscale 1 C{c:02}"),
            Product::AbiMeso2(c) => format!("ABI mesoscale 2 C{c:02}"),
            Product::AbiMetadata => "ABI metadata".into(),
            Product::GlmMetadata => "GLM metadata".into(),
            Product::Glm(1) => "GLM events".into(),
            Product::Glm(2) => "GLM flashes".into(),
            Product::Glm(_) => "GLM groups".into(),
            Product::SuviMetadata => "SUVI metadata".into(),
            Product::Suvi(b) => format!("SUVI {}", SUVI[usize::from(b.min(5))]),
            Product::Information => "GRB information".into(),
            Product::Unknown => "unknown".into(),
        }
    }

    /// For an ABI picture: the full size (width, height) in pixels and the reduction f (every f-th row and column is kept).
    pub fn abi_geometry(self) -> Option<(u32, u32, u32)> {
        let (ch, w_km, h_km) = match self {
            Product::AbiFullDisc(c) => (c, 10_848.0, 10_848.0),
            Product::AbiConus(c) => (c, 5000.0, 3000.0),
            Product::AbiMeso1(c) | Product::AbiMeso2(c) => (c, 1000.0, 1000.0),
            _ => return None,
        };
        let r: f64 = match ch {
            1 | 3 => 1.0,
            2 => 0.5,
            _ => 2.0,
        };
        let f = (4.0 / r).round() as u32;
        Some(((w_km / r) as u32, (h_km / r) as u32, f))
    }
}

// ---------------------------------------------------------------------------------------------------------------------------------
// The decoder of packets, payloads and pictures.

/// One picture of a product, kept at its reduced size.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Canvas {
    /// Width (reduced).
    pub width: u32,
    /// Height (reduced).
    pub height: u32,
    /// Every f-th row and column of the full picture is kept.
    pub f: u32,
    /// 8-bit samples, row by row.
    pub pixels: Vec<u8>,
    /// Rows that hold data.
    pub rows: Vec<bool>,
    /// Rows that hold data, counted.
    pub lines: u32,
    /// Signal time of the last block.
    pub updated: f64,
}

/// Counters of [`GrbDecoder`].
#[derive(Clone, Debug, Default, PartialEq)]
pub struct GrbStats {
    /// Packets taken.
    pub packets: u64,
    /// Packets with a bad CRC.
    pub crc_bad: u64,
    /// Payloads finished.
    pub payloads: u64,
    /// Picture blocks placed.
    pub blocks: u64,
    /// JPEG 2000 blocks that failed.
    pub j2k_failed: u64,
    /// The message of the last failure.
    pub j2k_error: String,
    /// Generic payloads given out as files.
    pub files: u64,
    /// Payloads of other variants or compressions.
    pub other: u64,
    /// Packets of each APID.
    pub by_apid: BTreeMap<u16, u64>,
}

#[derive(Clone, Debug, Default)]
struct Payload {
    secondary: [u8; 8],
    data: Vec<u8>,
    spoilt: bool,
    time: f64,
}

/// The payload layer and pictures of GRB, without the graph. See the [module documentation](self).
#[derive(Clone, Debug, Default)]
pub struct GrbDecoder {
    payloads: BTreeMap<(u8, u16), Payload>,
    /// The pictures by APID.
    pub pictures: BTreeMap<u16, Canvas>,
    /// The counters.
    pub stats: GrbStats,
    /// The APID of the picture updated last.
    pub newest: Option<u16>,
    /// The GLM store.
    pub glm: Glm,
    /// The last sequence count of each GLM APID by (virtual channel, APID).
    glm_seq: BTreeMap<(u8, u16), u16>,
}

/// What [`GrbDecoder::push`] gives out.
#[derive(Clone, Debug, PartialEq)]
pub enum GrbOut {
    /// Rows of a picture.
    Strip(Strip),
    /// A generic payload.
    File(SatFile),
}

fn be32(b: &[u8], at: usize) -> u32 {
    u32::from_be_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]])
}

/// The payload variant of a secondary header: (byte 6 AND 7) x 4 + (byte 7 shifted right 6).
pub fn variant(secondary: &[u8; 8]) -> u8 {
    (secondary[6] & 7) * 4 + (secondary[7] >> 6)
}

impl GrbDecoder {
    /// Takes one packet (primary header included) of virtual channel `vc` at signal time `time`.
    pub fn push(&mut self, vc: u8, p: &[u8], time: f64, out: &mut impl FnMut(GrbOut)) {
        if p.len() < MIN_PACKET {
            return;
        }
        let apid = u16::from_be_bytes([p[0], p[1]]) & 0x7FF;
        let flags = p[2] >> 6;
        self.stats.packets += 1;
        *self.stats.by_apid.entry(apid).or_default() += 1;
        let n = p.len();
        let crc_ok = CRC32_ISO_HDLC.checksum(&p[..n - 4]) == be32(p, n - 4);
        let key = (vc, apid);
        if !crc_ok {
            self.stats.crc_bad += 1;
            if let Some(pl) = self.payloads.get_mut(&key) {
                pl.spoilt = true;
            }
            return;
        }
        let is_glm = glm::is_glm(apid);
        if is_glm && !self.glm_sequence(key, p, flags) {
            return;
        }
        let mut secondary = [0u8; 8];
        secondary.copy_from_slice(&p[6..14]);
        let user = &p[14..n - 4];
        match flags {
            0b01 | 0b11 => {
                if let Some(old) = self.payloads.remove(&key) {
                    if is_glm {
                        // A GLM payload without its last packet is dropped, not finished.
                        self.glm.stats.dropped += 1;
                    } else {
                        self.finish(apid, old, out);
                    }
                }
                let pl = Payload {
                    secondary,
                    data: user.to_vec(),
                    spoilt: false,
                    time,
                };
                if flags == 0b11 {
                    self.finish(apid, pl, out);
                } else {
                    self.payloads.insert(key, pl);
                }
            }
            _ => {
                let Some(pl) = self.payloads.get_mut(&key) else {
                    return;
                };
                pl.data.extend_from_slice(user);
                pl.time = time;
                if flags == 0b10 {
                    if let Some(pl) = self.payloads.remove(&key) {
                        self.finish(apid, pl, out);
                    }
                }
            }
        }
    }

    /// Checks the sequence count of a good packet of a GLM APID against the one before (4.5.1). A gap counts the packets
    /// lost and drops the payload in progress, or the rest of a payload whose start was lost. Returns false for a duplicate.
    fn glm_sequence(&mut self, key: (u8, u16), p: &[u8], flags: u8) -> bool {
        let seq = u16::from_be_bytes([p[2], p[3]]) & 0x3FFF;
        let Some(prev) = self.glm_seq.insert(key, seq) else {
            return true;
        };
        if seq == prev {
            return false;
        }
        let lost = seq.wrapping_sub(prev).wrapping_sub(1) & 0x3FFF;
        if lost > 0 {
            self.glm.stats.packets_lost += u64::from(lost);
            if self.payloads.remove(&key).is_some() || flags & 1 == 0 {
                self.glm.stats.dropped += 1;
            }
        }
        true
    }

    fn finish(&mut self, apid: u16, pl: Payload, out: &mut impl FnMut(GrbOut)) {
        if pl.spoilt {
            if glm::is_glm(apid) {
                self.glm.stats.dropped += 1;
            }
            return;
        }
        self.stats.payloads += 1;
        match variant(&pl.secondary) {
            2 | 3 => self.block(apid, &pl.data, pl.time, out),
            0 => {
                let d = &pl.data;
                if d.len() < GENERIC_HEADER {
                    self.stats.other += 1;
                    return;
                }
                if glm::is_glm(apid) && self.glm.take(apid, d) == Taken::Decoded {
                    return;
                }
                let seconds = be32(d, 1);
                let ext = if (0x301..=0x303).contains(&apid) {
                    "bin"
                } else {
                    "xml"
                };
                self.stats.files += 1;
                out(GrbOut::File(SatFile {
                    name: format!("{apid:03X}_{seconds}.{ext}"),
                    kind: FileKind::GrbGeneric,
                    bytes: Arc::from(&d[GENERIC_HEADER..]),
                    parts: 0,
                    received: 0,
                    complete: true,
                    time: pl.time,
                }));
            }
            _ => self.stats.other += 1,
        }
    }

    fn block(&mut self, apid: u16, d: &[u8], time: f64, out: &mut impl FnMut(GrbOut)) {
        if d.len() < BLOCK_HEADER || d[0] != 1 {
            self.stats.other += 1;
            return;
        }
        let row_offset = u32::from_be_bytes([0, d[11], d[12], d[13]]);
        let (ulx, uly) = (be32(d, 14), be32(d, 18));
        let (bh, bw) = (be32(d, 22), be32(d, 26));
        let len = (be32(d, 30) as usize).min(d.len() - BLOCK_HEADER);
        let pic = match j2k::decode(&d[BLOCK_HEADER..BLOCK_HEADER + len]) {
            Ok(p) => p,
            Err(e) => {
                self.stats.j2k_failed += 1;
                self.stats.j2k_error = e.to_string();
                return;
            }
        };
        let Some(samples) = pic.components.first() else {
            self.stats.j2k_failed += 1;
            return;
        };
        let product = Product::of(apid);
        let (full_w, full_h, f) = match product.abi_geometry() {
            Some(g) => g,
            None => (bw.max(pic.width), bh.max(pic.height), 1),
        };
        // Header values are not trusted: positions are 64-bit and a picture is held to MAX_SIDE.
        let y0 = u64::from(uly) + u64::from(row_offset);
        let x0 = u64::from(ulx);
        let max_h = (y0 + u64::from(pic.height)).max(u64::from(full_h));
        let too_many = product.abi_geometry().is_none() && u64::from(full_w) * max_h > MAX_PIXELS;
        if full_w > MAX_SIDE || max_h > u64::from(MAX_SIDE) || too_many {
            self.stats.other += 1;
            return;
        }
        let c = self.pictures.entry(apid).or_insert_with(|| {
            let (w, h) = (full_w.div_ceil(f), full_h.div_ceil(f));
            Canvas {
                width: w,
                height: h,
                f,
                pixels: vec![0; (w as usize) * (h as usize)],
                rows: vec![false; h as usize],
                ..Default::default()
            }
        });
        // A SUVI picture takes the larger of the header's and the decoded height.
        if product.abi_geometry().is_none() && c.f == 1 && max_h > u64::from(c.height) {
            let h = max_h as u32;
            c.pixels.resize(c.width as usize * h as usize, 0);
            c.rows.resize(h as usize, false);
            c.height = h;
        }
        let shift = u32::from(pic.precision.saturating_sub(8));
        let f64_ = u64::from(c.f);
        let (mut r_lo, mut r_hi) = (u32::MAX, 0u32);
        let (mut c_lo, mut c_hi) = (u32::MAX, 0u32);
        for y in 0..pic.height {
            let gy = y0 + u64::from(y);
            if gy % f64_ != 0 || gy / f64_ >= u64::from(c.height) {
                continue;
            }
            let ry = (gy / f64_) as u32;
            for x in 0..pic.width {
                let gx = x0 + u64::from(x);
                if gx % f64_ != 0 || gx / f64_ >= u64::from(c.width) {
                    continue;
                }
                let rx = (gx / f64_) as u32;
                let v = u32::from(samples[(y * pic.width + x) as usize]) >> shift;
                c.pixels[ry as usize * c.width as usize + rx as usize] = v.min(255) as u8;
                c_lo = c_lo.min(rx);
                c_hi = c_hi.max(rx);
            }
            if !c.rows[ry as usize] {
                c.rows[ry as usize] = true;
                c.lines += 1;
            }
            r_lo = r_lo.min(ry);
            r_hi = r_hi.max(ry);
        }
        c.updated = time;
        self.stats.blocks += 1;
        self.newest = Some(apid);
        if r_lo > r_hi || c_lo > c_hi {
            return;
        }
        let (rows, width) = (r_hi - r_lo + 1, c_hi - c_lo + 1);
        let mut samples = Vec::with_capacity((rows * width) as usize);
        for ry in r_lo..=r_hi {
            let at = (ry * c.width + c_lo) as usize;
            samples.extend_from_slice(&c.pixels[at..at + width as usize]);
        }
        out(GrbOut::Strip(Strip {
            picture: u32::from(apid),
            ..Strip::new(1, r_lo, c_lo, rows as u16, width, Arc::from(samples))
        }));
    }
}

// ---------------------------------------------------------------------------------------------------------------------------------
// The components.

/// One row of the products table.
#[derive(Clone, Debug, PartialEq)]
pub struct ProductRow {
    /// The APID.
    pub apid: u16,
    /// Rows of its picture.
    pub lines: u32,
    /// Signal time it was last updated.
    pub updated: f64,
}

/// The report of [`GrbComponent`].
#[derive(Clone, Debug, Default, PartialEq)]
pub struct GrbReport {
    /// The counters.
    pub stats: GrbStats,
    /// The pictures.
    pub products: Vec<ProductRow>,
    /// The APID of the picture updated last.
    pub newest: Option<u16>,
    /// The GLM counters and the newest flashes and groups.
    pub glm: GlmReport,
}

impl Item for GrbReport {
    const KIND: &'static str = "satimg.grb_report";
}

/// The `clear` command of GRB.
pub const CLEAR: Command =
    Command::new("clear", "Clear").help("Forgets the pictures, the payloads and the counters.");

sdrkit_core::commands! {
    /// The commands of [`GrbComponent`].
    pub enum GrbCommand {
        /// Forgets everything.
        Clear = CLEAR,
    }
}

/// Whole packets in; picture strips, generic payload files and the report out.
#[derive(Default)]
pub struct GrbComponent {
    dec: GrbDecoder,
}

impl Component for GrbComponent {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "satimg.grb",
            vec![Port::typed::<WholePacket>("in")],
            vec![
                Port::typed::<Strip>("strips").rate_from(RateFrom::Fixed(0.0)),
                Port::typed::<SatFile>("files").rate_from(RateFrom::Fixed(0.0)),
                Port::typed::<GrbReport>("report").rate_from(RateFrom::Fixed(0.0)),
            ],
        )
    }
    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        Ok(vec![
            Format::new(Kind::typed::<Strip>(), 0.0),
            Format::new(Kind::typed::<SatFile>(), 0.0),
            Format::new(Kind::typed::<GrbReport>(), 0.0),
        ])
    }
    fn process(&mut self, io: &mut Io) {
        let mut strips = Vec::new();
        let mut files = Vec::new();
        for p in io.items::<WholePacket>(0) {
            let p = &p.value;
            self.dec.push(p.vcid, &p.bytes, p.time, &mut |o| match o {
                GrbOut::Strip(s) => strips.push(Timed {
                    time: p.time,
                    value: s,
                }),
                GrbOut::File(f) => files.push(Timed {
                    time: p.time,
                    value: f,
                }),
            });
        }
        io.items_out::<Strip>(0).extend(strips);
        io.items_out::<SatFile>(1).extend(files);
        if io.tick() {
            let r = GrbReport {
                stats: self.dec.stats.clone(),
                products: self
                    .dec
                    .pictures
                    .iter()
                    .map(|(&apid, c)| ProductRow {
                        apid,
                        lines: c.lines,
                        updated: c.updated,
                    })
                    .collect(),
                newest: self.dec.newest,
                glm: self.dec.glm.report(),
            };
            io.items_out::<GrbReport>(2).push(Timed {
                time: 0.0,
                value: r,
            });
        }
    }
    fn apply(&mut self, id: SettingId, _: &Value) -> Result<Applied, SettingError> {
        Err(SettingError::NoSuchId { id })
    }
    fn values(&self) -> Values {
        no_values()
    }
    fn commands(&self) -> &Commands {
        GrbCommand::commands()
    }
    fn invoke(&mut self, id: CommandId, _: &Values) -> Result<Option<Record>, CommandError> {
        match GrbCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            GrbCommand::Clear => self.dec = GrbDecoder::default(),
        }
        Ok(None)
    }
}

// ---------------------------------------------------------------------------------------------------------------------------------
// Settings.

/// The symbol rate (the default is the GRB rate, not confirmed by a public document).
pub const SYMBOL_RATE: Setting<field::Hz> = s2::symbol_rate(LINK_SYMBOL_RATE);
/// The roll-off.
pub const ROLL_OFF: Setting<field::Choice<RollOffChoice>> = s2::roll_off(RollOffChoice::R25);
/// The virtual channels read (not confirmed by a public document).
pub const CHANNELS: Setting<field::Text> = Setting::new("channels", "Virtual channels", "", "5,6")
    .help("Virtual channel numbers with commas between them. Empty: every channel but 63.")
    .advanced();
/// The product shown: an APID, or -1 for the one updated last. A front end offers the APIDs of the rows of
/// `grb_products`, the pictures received.
pub const PRODUCT: Setting<field::I64> = Setting::new("product", "Product shown", "", -1)
    .range(-1.0, 2047.0, 1.0)
    .help("An APID, or -1 for the picture updated last.")
    .role(sdrkit_core::SettingRole::PictureSource)
    .refers("grb_products", "apid");

sdrkit_core::settings! {
    /// The settings of `goes_grb`.
    pub enum GrbSetting {
        /// The symbol rate.
        SymbolRate = SYMBOL_RATE,
        /// The roll-off.
        RollOff = ROLL_OFF,
        /// The carrier.
        CarrierOffset = CARRIER_OFFSET,
        /// The product shown.
        Product = PRODUCT,
        /// The virtual channels read.
        Channels = CHANNELS,
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
    }
}

sdrkit_core::settings! {
    /// The settings of [`GrbDisplay`].
    pub enum GrbDisplaySetting {
        /// The symbol rate.
        SymbolRate = SYMBOL_RATE,
        /// The product shown.
        Product = PRODUCT,
    }
}

// ---------------------------------------------------------------------------------------------------------------------------------
// The display.

fn display_declaration() -> ResultsDecl {
    ResultsDecl::new()
        .with_fields(CaduFields::SHAPE.fields)
        .with_fields(GrbFields::SHAPE.fields)
        .with_fields(GlmFields::SHAPE.fields)
        .with_table(GRB_PRODUCTS.def())
        .with_table(GRB_PACKETS_BY_APID.def())
        .with_table(UHRIT_FRAMES.def())
        .with_table(GLM_FLASHES.def())
        .with_table(GLM_GROUPS.def())
}

/// What `goes_grb` publishes.
pub fn declaration() -> ResultsDecl {
    crate::typed::shape(ID).results_decl()
}

const NOTE_TEXT: &str = "Experimental: not confirmed on a real signal. Not checked against a public document: the symbol \
    rate 8 665 938 sym/s and roll-off 0.25, the CADUs of 2048 bytes in the data field without randomiser or Reed-Solomon, the \
    picture block header, the APIDs other than GLM and the 4 km reduction of ABI pictures. The guide's secondary header is not \
    consistent on the width of the payload variant (5 bits in its table, 3 in its text); the stack reads 5. GLM: flashes and groups are \
    decoded, events are counted; compressed GLM payloads (SZIP or JPEG 2000) are saved as files, not decoded; the GLM \
    metadata is saved as a file and only the event longitude offset is read from it; the guide gives the group record as 28 \
    bytes but describes 24 of them, so the other 4 are not read; event longitudes wait for a metadata payload; GLM times \
    count no leap seconds. SUVI colour is not composed.";

/// The display of `goes_grb`.
pub struct GrbDisplay {
    published: Published,
    symbol_rate: f64,
    product: i64,
    summary: String,
    hint: String,
    text: String,
    key: String,
    frames: FieldFramesReport,
}

impl Default for GrbDisplay {
    fn default() -> Self {
        GrbDisplay {
            published: super::display::published(display_declaration()),
            symbol_rate: LINK_SYMBOL_RATE,
            product: -1,
            summary: String::new(),
            hint: String::new(),
            text: String::new(),
            key: String::new(),
            frames: FieldFramesReport::default(),
        }
    }
}

impl Component for GrbDisplay {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::sink(
            "satimg.grb_display",
            vec![
                Port::typed::<LinkState>("link"),
                Port::typed::<FieldFramesReport>("frames"),
                Port::typed::<GrbReport>("grb"),
            ],
        )
        .with_settings(GrbDisplaySetting::schema())
    }
    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        Ok(vec![])
    }
    fn process(&mut self, io: &mut Io) {
        if !io.tick() {
            return;
        }
        let link = link_of(io, 0).unwrap_or_default();
        let fr = last::<FieldFramesReport>(io, 1);
        let g = last::<GrbReport>(io, 2).cloned().unwrap_or_default();
        let state = cadu_state(&link, fr);
        let shown = if self.product >= 0 {
            Some(self.product as u16)
        } else {
            g.newest
        };
        let lines = shown
            .and_then(|a| g.products.iter().find(|r| r.apid == a))
            .map_or(0, |r| r.lines);
        let st = &g.stats;
        let p = &mut self.published;
        if let Some(fr) = fr {
            self.frames.clone_from(fr);
            for (vc, &n) in fr.per_vc.iter().enumerate().filter(|(_, n)| **n > 0) {
                self.key.clear();
                let _ = write!(self.key, "{vc}");
                let row = UhritFrameRow {
                    vcid: vc as i64,
                    frames: n as i64,
                };
                UHRIT_FRAMES.put(p, &self.key, &row);
            }
        }
        p.put(&frame_fields::set(&self.frames, &mut self.text));
        for r in &g.products {
            self.key.clear();
            let _ = write!(self.key, "{}", r.apid);
            let name = Product::of(r.apid).name();
            let row = GrbProductRow {
                apid: i64::from(r.apid),
                name: &name,
                updated: Seconds(r.updated),
            };
            GRB_PRODUCTS.put(p, &self.key, &row);
        }
        let mut top: Vec<(&u16, &u64)> = st.by_apid.iter().collect();
        top.sort_by(|a, b| b.1.cmp(a.1));
        for (apid, n) in top.into_iter().take(32) {
            self.key.clear();
            let _ = write!(self.key, "{apid}");
            let name = Product::of(*apid).name();
            let row = GrbApidRow {
                apid: i64::from(*apid),
                name: &name,
                packets: *n as i64,
            };
            GRB_PACKETS_BY_APID.put(p, &self.key, &row);
        }
        publish_glm(p, &g.glm, &mut self.key);
        self.text.clear();
        if st.j2k_failed > 0 {
            let _ = write!(
                self.text,
                "{} JPEG 2000 blocks failed: {}",
                st.j2k_failed, st.j2k_error
            );
        } else if let Some(m) = link.modcod.filter(|_| link.state >= 2) {
            let _ = write!(self.text, "MODCOD {m}");
        }
        s2::hint(&link, self.symbol_rate, &self.text, &mut self.hint);
        let shown = shown.map_or(String::new(), |a| Product::of(a).name());
        p.put(&GrbFields {
            packets_crc_bad: st.crc_bad as i64,
            payloads: st.payloads as i64,
            picture_blocks: st.blocks as i64,
            j2k_failures: st.j2k_failed as i64,
            files: st.files as i64,
            picture_lines: i64::from(lines),
            shown_product: &shown,
            hint: &self.hint,
            note: NOTE_TEXT,
        });
        self.summary.clear();
        let frames = fr.map_or(0, |f| f.cadus);
        let _ = match state {
            3 => write!(
                self.summary,
                "GOES GRB, {frames} frames, {} packets, {} files",
                st.packets, st.files
            ),
            1 => write!(self.summary, "GOES GRB, carrier found, no frames yet"),
            _ => write!(self.summary, "GOES GRB, searching"),
        };
        s2::write_status(p, &link, state, &self.summary);
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match GrbDisplaySetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            GrbDisplaySetting::SymbolRate => self.symbol_rate = SYMBOL_RATE.read(id, v)?,
            GrbDisplaySetting::Product => self.product = PRODUCT.read(id, v)?,
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([
            (GrbDisplaySetting::SymbolRate, Value::Hz(self.symbol_rate)),
            (GrbDisplaySetting::Product, Value::I64(self.product)),
        ])
    }
    /// The frames and the packets counted are of the carrier of the frequency before. The list of products stays
    /// where `kept` holds it.
    fn forget(&mut self, kept: &sdrkit_graph::Kept) {
        self.frames = FieldFramesReport::default();
        self.published.clear_rows(UHRIT_FRAMES.def());
        self.published.clear_rows(GRB_PACKETS_BY_APID.def());
        for t in [GRB_PRODUCTS.def(), GLM_FLASHES.def(), GLM_GROUPS.def()] {
            if !kept.table(t.key) {
                self.published.clear_rows(t);
            }
        }
    }
    fn results(&self) -> Option<&Results> {
        Some(self.published.results())
    }
}

/// Writes the GLM counters, the newest flashes and the newest groups. A row key is the product time in microseconds since
/// J2000 and the identifier, so a row written again replaces itself.
fn publish_glm(p: &mut Published, g: &GlmReport, key: &mut String) {
    let st = &g.stats;
    p.put(&GlmFields {
        glm_payloads: st.payloads as i64,
        glm_dropped: st.dropped as i64,
        glm_packets_lost: st.packets_lost as i64,
        glm_not_decoded: st.not_decoded as i64,
        glm_bad_length: st.bad_length as i64,
        glm_group_bad_length: st.group_bad_length as i64,
        flash_count: st.flashes as i64,
        group_count: st.groups as i64,
        event_count: st.events as i64,
        glm_rejected: st.rejected as i64,
        newest_flash: g.newest_flash.map(Time),
        flash_rate: g.rate_per_minute,
    });
    for e in &g.flashes {
        let f = &e.flash;
        key.clear();
        let _ = write!(key, "{}-{}", e.product, f.id);
        let row = GlmFlashRow {
            flash: i64::from(f.id),
            time: Time(f.first),
            duration: Seconds(f.last - f.first),
            position: (f.lat, f.lon),
            area_km2: f.area_km2,
            energy_fj: f.energy_j.map(|j| j * 1e15),
            quality: f.quality,
            groups: i64::from(e.groups),
            events: i64::from(e.events),
        };
        GLM_FLASHES.put(p, key, &row);
    }
    for e in &g.groups {
        let r = &e.group;
        key.clear();
        let _ = write!(key, "{}-{}", e.product, r.id);
        let row = GlmGroupRow {
            group: i64::from(r.id),
            time: Time(r.time),
            position: (r.lat, r.lon),
            area_km2: r.area_km2,
            energy_fj: r.energy_j.map(|j| j * 1e15),
            flash: i64::from(r.parent),
            quality: r.quality,
            events: i64::from(e.events),
        };
        GLM_GROUPS.put(p, key, &row);
    }
}

// ---------------------------------------------------------------------------------------------------------------------------------
// The graph and the descriptor.

/// The parts of the DVB-S2 receiver that GRB takes.
const PARTS: Dvbs2Parts = Dvbs2Parts {
    ts: false,
    field: true,
};

/// The GRB graph.
pub fn builder() -> GraphBuilder {
    let channels = (1u64 << 5) | (1 << 6);
    attach(GraphBuilder::new(), PARTS)
        .add(
            "frames",
            FieldFramesComponent::new(FieldFrames {
                frame_crc: false,
                channels,
                uncounted_secondary: 0,
            }),
        )
        .add("grb", GrbComponent::default())
        .add("display", GrbDisplay::default())
        .connect("s2_fec.field", "frames.in")
        .connect("frames.packets", "grb.in")
        .connect("s2_report.link", "display.link")
        .connect("frames.report", "display.frames")
        .connect("grb.report", "display.grb")
        .output("grb.strips")
        .output("grb.files")
        .status_from("display")
        .clock(REPORT_S)
}

/// The GRB receiver with the settings of `cfg`.
///
/// # Errors
///
/// The errors of [`GraphBuilder::build`] and of [`sdrkit_graph::StackGraphBuilder::build`], among them a refused setting.
pub fn receiver(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    let mut g = builder()
        .catalog(&Catalog::of(cfg)?)
        .declare(&declaration())
        .build()?;
    g.set_max_items(1 << 20);
    let mut b = StackGraph::of(crate::typed::shape(ID), g)
        .min_rate(2e6, "GOES GRB needs more samples")
        .command("clear", sdrkit_graph::CommandTarget::every("clear"));
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
    b.route("product", &["display.product"])?
        .route("channels", &["frames.channels"])?
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
    GrbSetting::schema()
}

/// GOES-R Rebroadcast.
pub static DESCRIPTOR: StackDescriptor = StackDescriptor {
    id: ID,
    name: "GOES-R Rebroadcast",
    group: crate::GROUP,
    blurb: "GOES-R Rebroadcast on L band (DVB-S2, 8.67 Msym/s): ABI and SUVI pictures, GLM lightning flashes, and files.",
    tuning: Tuning {
        kind: Some(sdrkit_core::TuningKind::Channels),
        min_hz: 950e6,
        max_hz: 2200e6,
        ..Tuning::anywhere(1686.6e6, ASKED_RATE, 14.92e6, 1.5 * LINK_SYMBOL_RATE + 20e3)
    },
    submodes: &[],
    settings: schema,
    make_receiver,
    make_generator: Some(make_generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[],
    auto_gain: AutoGain::Radio,
    verify: Some(&crate::verify::GRB_HOOKS),
    benchmarks: None,
};

// ---------------------------------------------------------------------------------------------------------------------------------
// The test signal.

/// The spacecraft id of the test signal.
pub const TEST_SPACECRAFT: u8 = 0x12;
/// The APID of the test picture (ABI CONUS channel 13).
pub const TEST_APID: u16 = 0x0BC;
/// The APID of the test generic payload.
pub const TEST_GENERIC_APID: u16 = 0x580;
/// The seconds in the test headers.
pub const TEST_SECONDS: u32 = 123_456;
/// Lines of the test picture before it repeats.
pub const TEST_LINES: u32 = 40;
/// The largest packet data of the test signal.
pub const TEST_MAX_DATA: usize = 7000;

/// The 12-bit test pixel (x, y) of block `line`: ((x XOR (y + line)) x 91) AND FFF.
pub fn test_pixel(x: u32, y: u32, line: u32) -> u16 {
    (((x ^ (y + line)) * 91) & 0xFFF) as u16
}

/// The test block of `line`: 64 x 16 pixels of 12 bits, lossless JPEG 2000 (5-3 wavelet, 3 levels).
///
/// # Errors
///
/// The encoder's error.
pub fn test_block(line: u32) -> Result<Vec<u8>, sdrkit_codec_image::ImageError> {
    let pic = j2k::J2kPicture {
        width: 64,
        height: 16,
        precision: 12,
        components: vec![(0..16u32)
            .flat_map(|y| (0..64u32).map(move |x| test_pixel(x, y, line)))
            .collect()],
    };
    j2k::encode(
        &pic,
        &j2k::J2kEncode {
            lossless: true,
            levels: 3,
        },
    )
}

/// The payload of the picture block of `line`: the 34-byte header then the code stream.
pub fn block_payload(line: u32, stream: &[u8]) -> Vec<u8> {
    let mut h = vec![0u8; BLOCK_HEADER];
    h[0] = 1;
    h[1..5].copy_from_slice(&TEST_SECONDS.to_be_bytes());
    h[11..14].copy_from_slice(&(16 * line).to_be_bytes()[1..]);
    h[22..26].copy_from_slice(&16u32.to_be_bytes());
    h[26..30].copy_from_slice(&64u32.to_be_bytes());
    h[30..34].copy_from_slice(&(stream.len() as u32).to_be_bytes());
    h.extend_from_slice(stream);
    h
}

/// The generic payload of the test signal: the 21-byte header then 40 bytes 3C.
pub fn generic_payload(seconds: u32, data: &[u8]) -> Vec<u8> {
    let mut h = vec![0u8; GENERIC_HEADER];
    h[1..5].copy_from_slice(&seconds.to_be_bytes());
    h.extend_from_slice(data);
    h
}

/// The packets of one payload of variant `v` on `apid`, cut at [`TEST_MAX_DATA`] bytes, the sequence count running on.
pub fn payload_packets(apid: u16, v: u8, payload: &[u8], seq: &mut u16) -> Vec<Vec<u8>> {
    let mut sec = [0u8; 8];
    sec[6] = (v >> 2) & 7;
    sec[7] = (v & 3) << 6;
    let chunks: Vec<&[u8]> = payload.chunks(TEST_MAX_DATA).collect();
    let k = chunks.len();
    chunks
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let flags = match (i == 0, i + 1 == k) {
                (true, true) => 3,
                (true, false) => 1,
                (false, true) => 2,
                _ => 0,
            };
            let p = crc_packet(apid, flags, *seq, &sec, c, true);
            *seq = (*seq + 1) & 0x3FFF;
            p
        })
        .collect()
}

/// The CADUs of the test signal, one by one: virtual channel 5, spacecraft 12 hex, frame count from 0. Each time the
/// picture lines start again, the packets of one GLM product ([`glm::GlmTestSource`]) go first.
pub struct TestCadus {
    zs: ZoneStream,
    line: u32,
    seq: u16,
    counter: u32,
    blocks: Vec<Vec<u8>>,
    glm: glm::GlmTestSource,
}

impl TestCadus {
    /// The CADUs from the start.
    ///
    /// # Errors
    ///
    /// The JPEG 2000 encoder's error.
    pub fn new() -> Result<Self, sdrkit_codec_image::ImageError> {
        let blocks = (0..TEST_LINES)
            .map(test_block)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(TestCadus {
            zs: ZoneStream::default(),
            line: 0,
            seq: 0,
            counter: 0,
            blocks,
            glm: glm::GlmTestSource::default(),
        })
    }

    /// Appends the next CADU to `out`.
    pub fn next_cadu(&mut self, out: &mut Vec<u8>) {
        while self.zs.len() < ZONE_BYTES {
            if self.line == 0 {
                for p in self.glm.next_product() {
                    self.zs.push(&p);
                }
            }
            let pl = block_payload(self.line, &self.blocks[self.line as usize]);
            for p in payload_packets(TEST_APID, 2, &pl, &mut self.seq) {
                self.zs.push(&p);
            }
            let g = generic_payload(TEST_SECONDS, &[0x3C; 40]);
            for p in payload_packets(TEST_GENERIC_APID, 0, &g, &mut self.seq) {
                self.zs.push(&p);
            }
            self.line = (self.line + 1) % TEST_LINES;
        }
        let (fh, zone) = self.zs.take(ZONE_BYTES);
        out.extend(s2::cadu_2048(TEST_SPACECRAFT, 5, self.counter, fh, &zone));
        self.counter = (self.counter + 1) & 0xFF_FFFF;
    }
}

/// The GRB test signal: QPSK 9/10 (MODCOD 11), normal frames, no pilots, roll-off 0.25, a generic continuous stream of
/// [`TestCadus`] at the symbol rate of the setting `symbol_rate`.
///
/// # Errors
///
/// [`ConfigError`] if the symbol rate does not fit the sample rate or the test pictures do not encode.
pub fn generator(cfg: &GenConfig) -> Result<s2::S2Transmitter, ConfigError> {
    let rs = match cfg.settings.get(&SYMBOL_RATE) {
        r if r > 0.0 => r,
        _ => LINK_SYMBOL_RATE,
    };
    let mut cadus =
        TestCadus::new().map_err(|e| ConfigError::Stage(sdrkit_core::StageError::new(e)))?;
    s2::transmitter(
        cfg.rate_hz,
        rs,
        0.25,
        11,
        false,
        Box::new(sdrkit_proto_mpegts::source::NullSource),
        Some(Box::new(CaduStream::new(move |out: &mut Vec<u8>| {
            cadus.next_cadu(out)
        }))),
    )
}
