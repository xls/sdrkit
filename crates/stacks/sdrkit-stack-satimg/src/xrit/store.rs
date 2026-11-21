//! The product store of the xRIT stacks: files become products (images joined from their segments, compressed
//! pictures, texts and data), classified and kept newest first within the caps. JPEG 2000 segments are decoded on the helper
//! pool when the input is live, and in line for a recording, so a test sees the same products at any speed.

use super::crypto::des_ecb_decrypt;
use super::zip::first_file;
use crate::items::{ProductItem, ProductKind, XritFile};
use sdrkit_codec_image::j2k::{self, J2kPicture};
use sdrkit_codec_image::ImageError;
use sdrkit_core::media::Codec;
use sdrkit_core::{Choice, StateBlob, StateError};
use sdrkit_graph::{Pool, Ticket};
use sdrkit_proto_ccsds::rice;
use sdrkit_proto_ccsds::xrit::headers::{
    Compression, Dialect, Headers, ImageStructure, NoaaCompression,
};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::sync::Arc;

/// The most products kept.
pub const MAX_PRODUCTS: usize = 80;
/// The most bytes of pictures and data kept.
pub const MAX_BYTES: usize = 256 << 20;
/// GK-2A encrypted segments waiting for a key.
pub const MAX_LOCKED: usize = 96;

/// The configuration of the store for a stack.
#[derive(Clone, Debug, PartialEq)]
pub struct StoreConfig {
    /// The header dialect.
    pub dialect: Dialect,
    /// The log tag ("HRIT", "LRIT").
    pub tag: String,
    /// The largest picture (width, height) or pixels (FY-4: 200 megapixels).
    pub max_side: u32,
    /// The largest picture in pixels.
    pub max_pixels: u64,
}

impl StoreConfig {
    /// GOES and Elektro-L.
    pub fn goes() -> Self {
        StoreConfig {
            dialect: Dialect::Goes,
            tag: "HRIT".into(),
            max_side: 12_000,
            max_pixels: 144_000_000,
        }
    }
    /// GK-2A.
    pub fn gk2a(tag: &str) -> Self {
        StoreConfig {
            dialect: Dialect::Gk2a,
            tag: tag.into(),
            max_side: 40_000,
            max_pixels: 1_600_000_000,
        }
    }
    /// FY-4.
    pub fn fy4() -> Self {
        StoreConfig {
            dialect: Dialect::Fy4,
            tag: "FY-4".into(),
            max_side: u32::MAX,
            max_pixels: 200_000_000,
        }
    }
}

/// One product.
#[derive(Clone, Debug)]
pub struct Product {
    /// Running id.
    pub id: u32,
    /// Kind.
    pub kind: ProductKind,
    /// Name.
    pub name: String,
    /// Virtual channel.
    pub vcid: u8,
    /// APID.
    pub apid: u16,
    /// The first file's headers.
    pub headers: Headers,
    /// Signal time it started.
    pub start: f64,
    /// Grows with every change.
    pub version: u32,
    /// Segments expected.
    pub segments_total: u32,
    /// Segments received.
    pub segments: BTreeSet<u32>,
    /// Picture size.
    pub width: u32,
    /// Picture height.
    pub height: u32,
    /// Rows received.
    pub rows: BTreeSet<u32>,
    /// The picture (8 bits), when it has a buffer.
    pub picture: Option<Vec<u8>>,
    /// Every segment arrived.
    pub complete: bool,
    /// A file had errors.
    pub errors: bool,
    /// The note shown in the list.
    pub note: String,
    /// A compressed picture as it came.
    pub compressed: Option<(Codec, Arc<[u8]>)>,
    /// Text (ZIP unpacked).
    pub text: Option<String>,
    /// The file name inside a ZIP.
    pub inner_name: Option<String>,
    /// The data field (other files, ZIP archives).
    pub data: Option<Arc<[u8]>>,
    /// GK-2A: the channel index of the name (0 to 15, -1).
    pub channel: i32,
    /// The join key (GK-2A name, FY-4 series).
    pub series: String,
    /// Encrypted segments waiting for their key.
    pub locked: Vec<XritFile>,
    /// Rows a segment covers (GK-2A and FY-4).
    pub segment_height: u32,
}

impl Product {
    fn new(id: u32, kind: ProductKind, name: String, f: &XritFile) -> Self {
        Product {
            id,
            kind,
            name,
            vcid: f.vcid,
            apid: f.apid,
            headers: f.headers.clone(),
            start: f.time,
            version: 0,
            segments_total: 1,
            segments: BTreeSet::new(),
            width: 0,
            height: 0,
            rows: BTreeSet::new(),
            picture: None,
            complete: false,
            errors: false,
            note: String::new(),
            compressed: None,
            text: None,
            inner_name: None,
            data: None,
            channel: -1,
            series: String::new(),
            locked: Vec::new(),
            segment_height: 0,
        }
    }
    /// The list item.
    pub fn item(&self) -> ProductItem {
        ProductItem {
            id: self.id,
            kind: self.kind,
            name: self.name.clone(),
            vcid: self.vcid,
            apid: self.apid,
            rows: self.rows.len() as u32,
            height: self.height,
            width: self.width,
            segments: (self.segments.len() as u32, self.segments_total),
            complete: self.complete,
            errors: self.errors,
            version: self.version,
            note: self.note.clone(),
        }
    }
    /// Bytes held.
    pub fn bytes(&self) -> usize {
        self.picture.as_ref().map_or(0, |p| p.len())
            + self.compressed.as_ref().map_or(0, |c| c.1.len())
            + self.data.as_ref().map_or(0, |d| d.len())
            + self.text.as_ref().map_or(0, |t| t.len())
    }
    /// The "Done" column: rows or segments for images, a size for others, "..." while receiving, "errors".
    pub fn done(&self) -> String {
        if self.errors && self.complete {
            return "errors".into();
        }
        if self.kind.is_image() && self.compressed.is_none() {
            if self.segments_total > 1 {
                return format!("{}/{}", self.segments.len(), self.segments_total);
            }
            return if self.complete {
                format!("{} rows", self.rows.len())
            } else {
                "...".into()
            };
        }
        let n = self.bytes();
        if n >= 1024 {
            format!("{:.1} kB", n as f64 / 1024.0)
        } else {
            format!("{n} B")
        }
    }
}

/// Classifies a file (the first rule that matches).
pub fn classify(h: &Headers, name: &str) -> ProductKind {
    let image = h.is_image();
    if h.file_type == 2 {
        ProductKind::AdminMessage
    } else if h.file_type == 130 {
        ProductKind::Dcs
    } else if h.file_type == 214
        || h.noaa.as_ref().is_some_and(|n| n.product == 9)
        || [".ZIS", ".TXT", "EMWIN"].iter().any(|s| name.contains(s))
    {
        ProductKind::Emwin
    } else if image
        && [
            "_LA_", "_ELA_", "_KO_", "CMIPM", "-M1", "-M2", "_M1_", "_M2_", "Meso",
        ]
        .iter()
        .any(|s| name.contains(s))
    {
        ProductKind::Mesoscale
    } else if image && ["CMIPF", "FD", "Full"].iter().any(|s| name.contains(s)) {
        ProductKind::FullDisk
    } else if image && ["CMIPC", "CONUS"].iter().any(|s| name.contains(s)) {
        ProductKind::Conus
    } else if image {
        ProductKind::Image
    } else if h.file_type == 1 {
        ProductKind::Text
    } else {
        ProductKind::Other
    }
}

/// The GK-2A channel names, in index order.
pub const GK2A_CHANNELS: [&str; 16] = [
    "VI004", "VI005", "VI006", "VI008", "NR013", "NR016", "SW038", "WV063", "WV069", "WV073",
    "IR087", "IR096", "IR105", "IR112", "IR123", "IR133",
];

/// The GK-2A channel name of a channel index (`None` for -1 and other values outside the list).
pub fn gk2a_channel_name(index: i32) -> Option<&'static str> {
    usize::try_from(index)
        .ok()
        .and_then(|i| GK2A_CHANNELS.get(i).copied())
}

/// The GK-2A channel index of a file name (-1 when it is not a channel image).
pub fn gk2a_channel(name: &str) -> i32 {
    if !name.starts_with("IMG_") {
        return -1;
    }
    name.split(['_', '.'])
        .find_map(|p| GK2A_CHANNELS.iter().position(|c| *c == p))
        .map_or(-1, |i| i as i32)
}

/// A parsed FY-4 file name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Fy4Name {
    /// "FY-4A".
    pub satellite: String,
    /// "AGRI".
    pub instrument: String,
    /// "DISK", "REGC", "REGE".
    pub region: String,
    /// "C012", "MULT".
    pub channel: String,
    /// "YYYYMMDDhhmmss".
    pub start: String,
}

/// Parses an FY-4 file name (13 fields separated by "_").
pub fn parse_fy4_name(name: &str) -> Option<Fy4Name> {
    let f: Vec<&str> = name.split('_').collect();
    if f.len() < 13 || f[0].len() != 5 || !f[0].starts_with("FY4") {
        return None;
    }
    let sat = format!("FY-4{}", &f[0][3..4]);
    Some(Fy4Name {
        satellite: sat,
        instrument: f[1].replace('-', " ").trim().to_string(),
        region: f[3].to_string(),
        channel: f[7].to_string(),
        start: f[9].to_string(),
    })
}

/// Where a JPEG 2000 segment goes.
#[derive(Clone, Debug)]
struct JobMeta {
    product: u32,
    first_line: u32,
    first_column: u32,
    header_bits: u32,
    name: String,
}

/// What a file did to the store.
#[derive(Clone, Debug, Default)]
pub struct Changes {
    /// Products changed (their list items).
    pub items: Vec<ProductItem>,
    /// Compressed pictures that completed.
    pub compressed: Vec<(u32, Codec, Arc<[u8]>, String)>,
    /// Texts that completed.
    pub texts: Vec<(u32, String, String, Option<String>)>,
    /// Products whose picture changed.
    pub pictures: Vec<u32>,
}

/// Counters.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct StoreStats {
    /// Files taken.
    pub files: u64,
    /// Files with errors.
    pub file_errors: u64,
    /// Encrypted files (FY-4) or locked segments seen (GK-2A).
    pub encrypted: u64,
    /// Image segments.
    pub segments: u64,
    /// Pictures complete.
    pub complete_pictures: u64,
    /// Segments waiting for a key now.
    pub locked_now: u64,
}

/// The store.
pub struct Store {
    cfg: StoreConfig,
    /// Products, oldest first.
    pub products: VecDeque<Product>,
    next_id: u32,
    keys: BTreeMap<u16, [u8; 8]>,
    /// Log lines waiting to be published.
    pub log: Vec<String>,
    /// Counters.
    pub stats: StoreStats,
    /// The helper pool for JPEG 2000 (live input only; `None` decodes in line).
    pub pool: Option<Pool>,
    jobs: Vec<(JobMeta, Ticket<Result<J2kPicture, ImageError>>)>,
}

impl Store {
    /// An empty store.
    pub fn new(cfg: StoreConfig) -> Self {
        Store {
            cfg,
            products: VecDeque::new(),
            next_id: 1,
            keys: BTreeMap::new(),
            log: Vec::new(),
            stats: StoreStats::default(),
            pool: None,
            jobs: Vec::new(),
        }
    }

    /// The configuration.
    pub fn config(&self) -> &StoreConfig {
        &self.cfg
    }

    /// Empties the products.
    pub fn clear(&mut self) {
        self.products.clear();
        self.jobs.clear();
    }

    /// Keys loaded.
    pub fn keys(&self) -> usize {
        self.keys.len()
    }

    /// Imports keys; returns how many were read. Segments waiting for them are decoded now.
    pub fn import_keys(&mut self, keys: BTreeMap<u16, [u8; 8]>, ch: &mut Changes) -> usize {
        let n = keys.len();
        self.keys.extend(keys);
        let mut waiting = Vec::new();
        for p in self.products.iter_mut() {
            waiting.append(&mut p.locked);
        }
        self.stats.locked_now = 0;
        for f in waiting {
            self.push(f, ch);
        }
        self.refresh_locked_notes(ch);
        n
    }

    fn refresh_locked_notes(&mut self, ch: &mut Changes) {
        let mut locked = 0;
        for p in self.products.iter_mut() {
            locked += p.locked.len();
            if p.locked.is_empty() && p.note.starts_with("encrypted (KMA key") {
                p.note.clear();
                p.version += 1;
                ch.items.push(p.item());
            }
        }
        self.stats.locked_now = locked as u64;
    }

    fn product_mut(&mut self, id: u32) -> Option<&mut Product> {
        self.products.iter_mut().find(|p| p.id == id)
    }

    /// The product with this id.
    pub fn product(&self, id: u32) -> Option<&Product> {
        self.products.iter().find(|p| p.id == id)
    }

    fn add_product(&mut self, p: Product) -> u32 {
        let id = p.id;
        self.products.push_back(p);
        while self.products.len() > MAX_PRODUCTS {
            self.products.pop_front();
        }
        let mut total: usize = self.products.iter().map(|p| p.bytes()).sum();
        while total > MAX_BYTES && self.products.len() > 1 {
            if let Some(p) = self.products.pop_front() {
                total -= p.bytes();
            }
        }
        id
    }

    fn new_id(&mut self) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    /// Takes a file.
    pub fn push(&mut self, mut f: XritFile, ch: &mut Changes) {
        self.stats.files += 1;
        if f.errors {
            self.stats.file_errors += 1;
        }
        let name = f.headers.annotation.clone().unwrap_or_default();
        // GK-2A: decrypt with the file's key, or keep it locked.
        let mut encrypted = false;
        if self.cfg.dialect == Dialect::Gk2a {
            if let Some(k) = f.headers.key.filter(|&k| k != 0) {
                match self.keys.get(&(k as u16)) {
                    Some(key) => {
                        let mut b = f.bytes.to_vec();
                        let h = f.headers.header_len as usize;
                        des_ecb_decrypt(key, &mut b[h..]);
                        f.bytes = Arc::from(b);
                    }
                    None => encrypted = true,
                }
            }
        }
        if self.cfg.dialect == Dialect::Fy4 && f.headers.key.is_some_and(|k| k != 0) {
            self.fy4_encrypted(&f, &name, ch);
            return;
        }
        let kind = classify(&f.headers, &name);
        let enc = if encrypted { ", encrypted" } else { "" };
        let errs = if f.errors { ", with errors" } else { "" };
        if self.cfg.dialect != Dialect::Fy4 {
            self.log.push(format!(
                "{}: {} {} (VC {}, APID {}, {} bytes{errs}{enc})",
                self.cfg.tag,
                kind.label(),
                name,
                f.vcid,
                f.apid,
                f.bytes.len()
            ));
        }
        if kind.is_image() {
            self.image(f, kind, name, encrypted, ch);
        } else {
            self.other(f, kind, name, ch);
        }
    }

    fn fy4_encrypted(&mut self, f: &XritFile, name: &str, ch: &mut Changes) {
        self.stats.encrypted += 1;
        let key = f.headers.key.unwrap_or(0);
        let id = self.new_id();
        let mut p = Product::new(
            id,
            classify(&f.headers, name),
            display_name(name, id, ProductKind::Other),
            f,
        );
        p.note =
            format!("Encrypted file (key {key}): kept as it came, it can not be decoded here.");
        p.errors = true;
        p.complete = true;
        p.data = Some(f.bytes.clone());
        ch.items.push(p.item());
        self.add_product(p);
        self.log
            .push(format!("FY-4: encrypted file {name} kept as received"));
    }

    fn other(&mut self, f: XritFile, kind: ProductKind, name: String, ch: &mut Changes) {
        let id = self.new_id();
        let mut p = Product::new(id, kind, display_name(&name, id, kind), &f);
        let data = f.headers.data(&f.bytes);
        let zip = f
            .headers
            .noaa
            .as_ref()
            .is_some_and(|n| n.compression == NoaaCompression::Zip)
            || data.starts_with(b"PK");
        p.complete = true;
        p.errors = f.errors;
        let textual = matches!(
            kind,
            ProductKind::AdminMessage | ProductKind::Dcs | ProductKind::Emwin | ProductKind::Text
        );
        if zip {
            p.data = Some(Arc::from(data));
            if let Some((inner, bytes)) = first_file(data) {
                p.text = Some(trim_text(&bytes));
                p.inner_name = Some(inner);
            } else {
                p.note = "picture not decoded: the ZIP archive could not be unpacked".into();
                p.errors = true;
            }
        } else if textual || (self.cfg.dialect == Dialect::Fy4 && printable(data)) {
            p.text = Some(trim_text(data));
            if self.cfg.dialect == Dialect::Fy4 {
                p.kind = ProductKind::Text;
            }
        } else {
            p.data = Some(Arc::from(data));
        }
        if self.cfg.dialect == Dialect::Fy4 {
            self.log
                .push(format!("FY-4: file {} ({} bytes)", name, data.len()));
        }
        if let Some(t) = &p.text {
            ch.texts
                .push((id, p.name.clone(), t.clone(), p.inner_name.clone()));
        }
        ch.items.push(p.item());
        self.add_product(p);
    }

    fn image(
        &mut self,
        f: XritFile,
        kind: ProductKind,
        name: String,
        encrypted: bool,
        ch: &mut Changes,
    ) {
        self.stats.segments += 1;
        let h = f.headers.clone();
        let img = h.image.clone().unwrap_or_default();
        let data = h.data(&f.bytes).to_vec();
        // Relayed JPEG and GIF pictures are kept as they came.
        let noaa_comp = h.noaa.as_ref().map(|n| n.compression);
        let is_j2k = j2k::find_stream(&data).is_some();
        if matches!(
            noaa_comp,
            Some(NoaaCompression::Jpeg) | Some(NoaaCompression::Gif)
        ) || (img.compression == Compression::Lossy
            && !is_j2k
            && self.cfg.dialect == Dialect::Goes)
        {
            let gif = noaa_comp == Some(NoaaCompression::Gif) || data.starts_with(b"GIF");
            let id = self.new_id();
            let mut p = Product::new(id, kind, display_name(&name, id, kind), &f);
            let codec = if gif { Codec::Gif } else { Codec::Jpeg };
            let bytes: Arc<[u8]> = Arc::from(data);
            p.compressed = Some((codec, bytes.clone()));
            p.width = img.columns as u32;
            p.height = img.lines as u32;
            p.complete = true;
            p.errors = f.errors;
            ch.compressed.push((id, codec, bytes, p.name.clone()));
            ch.items.push(p.item());
            self.add_product(p);
            return;
        }
        // Which product does this segment join, and where does it go?
        let (id, first_line, first_column) = self.join(&f, kind, &name, &img, ch);
        if encrypted {
            let key = h.key.unwrap_or(0);
            let locked_total: usize = self.products.iter().map(|p| p.locked.len()).sum();
            if let Some(p) = self.product_mut(id) {
                p.note = format!("encrypted (KMA key {key}), key not loaded");
                if locked_total < MAX_LOCKED {
                    p.locked.push(f);
                }
                p.version += 1;
                ch.items.push(p.item());
            }
            self.stats.encrypted += 1;
            self.stats.locked_now = self.products.iter().map(|p| p.locked.len() as u64).sum();
            return;
        }
        let bits = img.bits as u32;
        // 0: no compression, 1: Rice (the NOAA record wins over record 1), 2: another code.
        let comp = match (noaa_comp, img.compression) {
            (Some(NoaaCompression::None), _) => 0,
            (Some(NoaaCompression::Rice), _) => 1,
            (Some(_), _) => 2,
            (None, Compression::None) => 0,
            (None, Compression::Lossless) => 1,
            (None, _) => 2,
        };
        if is_j2k {
            let meta = JobMeta {
                product: id,
                first_line,
                first_column,
                header_bits: bits,
                name: name.clone(),
            };
            match &self.pool {
                Some(pool) => {
                    let ticket = pool.spawn(move || j2k::decode(&data));
                    self.jobs.push((meta, ticket));
                }
                None => {
                    let r = j2k::decode(&data).map_err(DecodeError::Image);
                    self.finish_j2k(meta, r, ch);
                }
            }
            self.mark_segment(id, &f, ch);
            return;
        }
        let rows = match comp {
            1 if self.cfg.dialect == Dialect::Goes => self.rice(&f, &img),
            0 => {
                // After a lost packet the offsets of what follows are unknown: rows up to the first gap only.
                let hl = f.headers.header_len as usize;
                let cut = f
                    .packet_index
                    .iter()
                    .enumerate()
                    .find(|(k, &i)| i as usize != *k)
                    .map(|(k, _)| (f.packet_starts[k] as usize).saturating_sub(hl));
                let usable = cut.map_or(&data[..], |c| &data[..c.min(data.len())]);
                plain_rows(usable, img.columns as usize, img.lines as usize, bits)
            }
            _ => {
                if let Some(p) = self.product_mut(id) {
                    p.note = "picture not decoded: unknown compression".into();
                    p.errors = true;
                }
                None
            }
        };
        if let Some((rows, short)) = rows {
            let w = img.columns as u32;
            self.place_rows(id, first_line, first_column, w, &rows, ch);
            if short {
                if let Some(p) = self.product_mut(id) {
                    p.errors = true;
                }
            }
        }
        self.mark_segment(id, &f, ch);
    }

    fn rice(&mut self, f: &XritFile, img: &ImageStructure) -> Option<(Vec<Vec<u8>>, bool)> {
        let h = &f.headers;
        let cols = img.columns as usize;
        let p = rice_params(h, img)?;
        let per_packet = h.rice.map_or(1, |r| (r.lines_per_packet as usize).max(1));
        let hl = h.header_len as usize;
        let mut rows: Vec<Vec<u8>> = Vec::with_capacity(img.lines as usize);
        let mut short = f.errors;
        let starts = &f.packet_starts;
        let shift = p.bits.saturating_sub(8);
        let mut line = vec![0u32; cols];
        // The header packet can carry the first line after the headers, or only the headers.
        let first_line_packet = if starts.len() > 1 && starts[1] as usize <= hl {
            1
        } else {
            0
        };
        for (k, &s) in starts.iter().enumerate() {
            // A lost packet is a lost line: the lines that follow keep their place.
            let idx = f.packet_index.get(k).copied().unwrap_or(k as u32) as usize;
            let want = idx.saturating_sub(first_line_packet) * per_packet;
            while rows.len() < want.min(img.lines as usize) {
                rows.push(Vec::new());
                short = true;
            }
            let start = if k == 0 { hl } else { s as usize };
            let end = starts.get(k + 1).map_or(f.bytes.len(), |&e| e as usize);
            if start >= end || start >= f.bytes.len() {
                continue;
            }
            let mut chunk = &f.bytes[start..end];
            for _ in 0..per_packet {
                if rows.len() >= img.lines as usize || chunk.is_empty() {
                    break;
                }
                line.fill(0);
                let used = match rice::decode(chunk, &p, &mut line) {
                    Ok(d) => {
                        if d.samples < cols {
                            short = true;
                        }
                        d.bytes()
                    }
                    Err(_) => {
                        short = true;
                        chunk.len()
                    }
                };
                rows.push(line.iter().map(|&v| (v >> shift).min(255) as u8).collect());
                // Several lines in one packet: the next line starts at the next byte.
                chunk = &chunk[used.min(chunk.len())..];
            }
        }
        if rows.len() < img.lines as usize {
            short = true;
        }
        Some((rows, short))
    }

    /// Finds or makes the product a segment joins, and where the segment goes.
    fn join(
        &mut self,
        f: &XritFile,
        kind: ProductKind,
        name: &str,
        img: &ImageStructure,
        ch: &mut Changes,
    ) -> (u32, u32, u32) {
        let h = &f.headers;
        let lines = img.lines as u32;
        let cols = img.columns as u32;
        match self.cfg.dialect {
            Dialect::Goes => {
                let seg = h.segment;
                if let Some(s) = seg.as_ref().filter(|s| s.total > 1) {
                    let found = self.products.iter().rev().find(|p| {
                        p.vcid == f.vcid
                            && p.headers
                                .segment
                                .as_ref()
                                .is_some_and(|q| q.image_id == s.image_id)
                            && !p.complete
                            && p.segments_total == s.total as u32
                    });
                    let id = match found {
                        Some(p) => p.id,
                        None => {
                            let id = self.new_id();
                            let mut p = Product::new(id, kind, display_name(name, id, kind), f);
                            p.segments_total = s.total as u32;
                            p.width = s.max_column as u32;
                            p.height = s.max_row as u32;
                            self.alloc(&mut p);
                            self.add_product(p);
                            id
                        }
                    };
                    return (id, s.start_line as u32, s.start_column as u32);
                }
                let id = self.new_id();
                let mut p = Product::new(id, kind, display_name(name, id, kind), f);
                p.width = cols;
                p.height = lines;
                self.alloc(&mut p);
                ch.items.push(p.item());
                self.add_product(p);
                (id, 0, 0)
            }
            Dialect::Gk2a => {
                let seg = h.segment.unwrap_or_default();
                let series = match name.rfind('_') {
                    Some(i) => name[..i].to_string(),
                    None => name.to_string(),
                };
                let total = (seg.total as u32).max(1);
                let found = self.products.iter().rev().find(|p| {
                    total > 1 && p.series == series && !p.complete && p.segments_total == total
                });
                let id = match found {
                    Some(p) => p.id,
                    None => {
                        let id = self.new_id();
                        let mut p = Product::new(id, kind, display_name(name, id, kind), f);
                        if total > 1 {
                            p.name = series.clone();
                        }
                        p.series = series;
                        p.segments_total = total;
                        p.width = cols;
                        p.height = lines * total;
                        p.segment_height = lines;
                        p.channel = gk2a_channel(name);
                        self.alloc(&mut p);
                        self.add_product(p);
                        id
                    }
                };
                let first = (seg.sequence.max(1) as u32 - 1) * lines;
                (id, first, 0)
            }
            Dialect::Fy4 => {
                let fy = img.fy4.clone().unwrap_or_default();
                let parsed = parse_fy4_name(name);
                let total = (fy.total_segments as u32).max(1);
                let series = match &parsed {
                    Some(n) => format!("{}/{}/{}", n.satellite, n.region, n.start),
                    None => format!("{}/{}", fy.satellite, fy.instrument),
                };
                let key = format!("{series}/C{:02}/{total}", fy.channel);
                let first = if fy.first_line > 0 {
                    fy.first_line as u32
                } else if fy.position > 0 {
                    (fy.position as u32 - 1) * lines
                } else {
                    (fy.segment.max(1) as u32 - 1) * lines
                };
                let found = self
                    .products
                    .iter()
                    .rev()
                    .find(|p| p.series == key)
                    .map(|p| {
                        let next = p.complete || p.width != cols || p.rows.contains(&first);
                        (p.id, next)
                    });
                let id = match found {
                    Some((id, false)) => id,
                    _ => {
                        let id = self.new_id();
                        let region = parsed
                            .as_ref()
                            .map(|n| n.region.clone())
                            .unwrap_or_default();
                        let kind = match region.as_str() {
                            "DISK" => ProductKind::FullDisk,
                            "REGC" | "REGE" => ProductKind::Mesoscale,
                            _ => ProductKind::Image,
                        };
                        let sat = parsed
                            .as_ref()
                            .map_or(fy.satellite.clone(), |n| n.satellite.clone());
                        let ins = parsed
                            .as_ref()
                            .map_or(fy.instrument.clone(), |n| n.instrument.clone());
                        let mut title = format!("{sat} {ins} C{:02}", fy.channel);
                        if !region.is_empty() {
                            title.push(' ');
                            title.push_str(&region);
                        }
                        if let Some(n) = &parsed {
                            if n.start.len() >= 12 {
                                let s = &n.start;
                                title.push_str(&format!(
                                    " {}-{}-{} {}:{}Z",
                                    &s[0..4],
                                    &s[4..6],
                                    &s[6..8],
                                    &s[8..10],
                                    &s[10..12]
                                ));
                            }
                        }
                        let mut p = Product::new(id, kind, title, f);
                        p.series = key;
                        p.segments_total = total;
                        p.width = cols;
                        p.height = lines * total;
                        p.segment_height = lines;
                        self.alloc(&mut p);
                        self.add_product(p);
                        id
                    }
                };
                (id, first, 0)
            }
        }
    }

    fn alloc(&self, p: &mut Product) {
        let px = p.width as u64 * p.height as u64;
        if p.width == 0
            || p.height == 0
            || p.width > self.cfg.max_side
            || p.height > self.cfg.max_side
            || px > self.cfg.max_pixels
        {
            if px > 0 {
                p.note = "too large for a picture buffer (listed as received)".into();
            }
            return;
        }
        p.picture = Some(vec![0u8; px as usize]);
    }

    fn place_rows(
        &mut self,
        id: u32,
        y0: u32,
        x0: u32,
        w: u32,
        rows: &[Vec<u8>],
        ch: &mut Changes,
    ) {
        let Some(p) = self.product_mut(id) else {
            return;
        };
        let pw = p.width as usize;
        let ph = p.height;
        if let Some(pic) = &mut p.picture {
            for (k, r) in rows.iter().enumerate() {
                let y = y0 + k as u32;
                if y >= ph {
                    break;
                }
                if r.is_empty() {
                    continue;
                }
                let x = (x0 as usize).min(pw);
                let n = (w as usize).min(pw - x).min(r.len());
                pic[y as usize * pw + x..y as usize * pw + x + n].copy_from_slice(&r[..n]);
                p.rows.insert(y);
            }
        } else {
            for (k, r) in rows.iter().enumerate() {
                if !r.is_empty() {
                    p.rows.insert(y0 + k as u32);
                }
            }
        }
        p.version += 1;
        ch.pictures.push(id);
    }

    fn mark_segment(&mut self, id: u32, f: &XritFile, ch: &mut Changes) {
        let h = &f.headers;
        let seq = match self.cfg.dialect {
            Dialect::Fy4 => h
                .image
                .as_ref()
                .and_then(|i| i.fy4.as_ref())
                .map_or(1, |i| i.segment.max(1) as u32),
            _ => h.segment.as_ref().map_or(1, |s| s.sequence.max(1) as u32),
        };
        let mut done = None;
        if let Some(p) = self.product_mut(id) {
            p.segments.insert(seq);
            p.errors |= f.errors;
            p.version += 1;
            if p.segments.len() as u32 >= p.segments_total && !p.complete {
                p.complete = true;
                done = Some(p.id);
            }
            ch.items.push(p.item());
        }
        if done.is_some() && self.jobs.is_empty() {
            self.stats.complete_pictures += 1;
        }
    }

    /// Takes the finished decodes of the helper pool (never waits).
    pub fn poll(&mut self, ch: &mut Changes) {
        let mut k = 0;
        while k < self.jobs.len() {
            match self.jobs[k].1.try_take() {
                Some(r) => {
                    let (meta, _) = self.jobs.swap_remove(k);
                    let r = r
                        .map_err(|_| DecodeError::Stopped)
                        .and_then(|r| r.map_err(DecodeError::Image));
                    self.finish_j2k(meta, r, ch);
                }
                None => k += 1,
            }
        }
    }

    /// Waits for the helper pool (for tests and the end of a recording, never on the decoding thread).
    pub fn wait(&mut self, ch: &mut Changes) {
        for (meta, t) in std::mem::take(&mut self.jobs) {
            let r = t
                .wait()
                .map_err(|_| DecodeError::Stopped)
                .and_then(|r| r.map_err(DecodeError::Image));
            self.finish_j2k(meta, r, ch);
        }
    }

    /// JPEG 2000 segments still decoding.
    pub fn decoding(&self) -> usize {
        self.jobs.len()
    }

    fn finish_j2k(
        &mut self,
        r: JobMeta,
        result: Result<J2kPicture, DecodeError>,
        ch: &mut Changes,
    ) {
        let tag = self.cfg.tag.clone();
        match result {
            Ok(pic) => {
                // To 8 bits: the depth of the header when it is 8 to 16 and not above the code stream's, else the code stream's.
                let precision = pic.precision as u32;
                let bits = if (8..=16).contains(&r.header_bits) && r.header_bits <= precision {
                    r.header_bits
                } else {
                    precision
                };
                let shift = bits.saturating_sub(8);
                let w = (pic.width as usize).max(1);
                let samples = pic.components.first().map_or(&[][..], |c| &c[..]);
                let rows: Vec<Vec<u8>> = samples
                    .chunks(w)
                    .map(|r| {
                        r.iter()
                            .map(|&v| ((v as u32) >> shift).min(255) as u8)
                            .collect()
                    })
                    .collect();
                if self.cfg.dialect == Dialect::Fy4 {
                    let seg = self
                        .product(r.product)
                        .map(|p| (p.segments.len() + 1, p.segments_total));
                    let (s, n) = seg.unwrap_or((1, 1));
                    self.log.push(format!(
                        "FY-4: {} segment {s} of {n} ({} x {}, {} bit)",
                        r.name, pic.width, pic.height, bits
                    ));
                }
                self.place_rows(
                    r.product,
                    r.first_line,
                    r.first_column,
                    pic.width,
                    &rows,
                    ch,
                );
            }
            Err(e) => {
                if let Some(p) = self.product_mut(r.product) {
                    p.errors = true;
                    p.note = format!("picture not decoded: {e}");
                    p.version += 1;
                    ch.items.push(p.item());
                }
                if self.cfg.dialect == Dialect::Fy4 {
                    self.log.push(format!(
                        "FY-4: a picture segment could not be decoded ({})",
                        r.name
                    ));
                } else {
                    self.log.push(format!(
                        "{tag}: a picture segment could not be decoded ({})",
                        r.name
                    ));
                }
            }
        }
    }

    /// The products for a saved state: newest first, up to 48 MB of pictures and data.
    pub fn save_state(&self) -> StateBlob {
        let mut products = Vec::new();
        let mut budget = 48usize << 20;
        for p in self.products.iter().rev() {
            let n = p.bytes();
            if n > budget {
                break;
            }
            budget -= n;
            products.push(SavedProduct {
                id: p.id,
                kind: p.kind.index(),
                name: p.name.clone(),
                vcid: p.vcid,
                apid: p.apid,
                width: p.width,
                height: p.height,
                rows: p.rows.iter().copied().collect(),
                segments: p.segments.iter().copied().collect(),
                segments_total: p.segments_total,
                complete: p.complete,
                errors: p.errors,
                note: p.note.clone(),
                text: p.text.clone(),
                inner_name: p.inner_name.clone(),
                compressed_gif: p.compressed.as_ref().map(|c| c.0 == Codec::Gif),
                compressed: p.compressed.as_ref().map(|c| hex(&c.1)),
                picture: p.picture.as_ref().map(|x| hex(x)),
                data: p.data.as_ref().map(|x| hex(x)),
            });
        }
        let bytes = serde_json::to_vec(&SavedStore { products }).unwrap_or_default();
        StateBlob::new(STATE_OWNER, STATE_VERSION, bytes)
    }

    /// Replaces the products with those of a state made by [`Store::save_state`]. Returns the number of products.
    ///
    /// # Errors
    ///
    /// The [`StateError`] of a state of another owner or version, and [`StateError::Damaged`] if a product does not read. The
    /// store then stays as it was.
    pub fn restore_state(&mut self, state: &StateBlob) -> Result<usize, StateError> {
        let bytes = state.open(STATE_OWNER, STATE_VERSION)?;
        let saved: SavedStore = serde_json::from_slice(bytes).map_err(|_| StateError::Damaged)?;
        let mut loaded = Vec::with_capacity(saved.products.len());
        for m in saved.products {
            let bytes = |t: Option<String>| -> Result<Option<Vec<u8>>, StateError> {
                t.map(|t| unhex(&t).ok_or(StateError::Damaged)).transpose()
            };
            let kind = ProductKind::from_index(m.kind).ok_or(StateError::Damaged)?;
            let headers = Headers {
                dialect: self.cfg.dialect,
                ..Headers::default()
            };
            let mut p = Product {
                id: m.id,
                kind,
                name: m.name,
                vcid: m.vcid,
                apid: m.apid,
                headers,
                start: 0.0,
                version: 1,
                segments_total: m.segments_total,
                segments: m.segments.into_iter().collect(),
                width: m.width,
                height: m.height,
                rows: m.rows.into_iter().collect(),
                picture: bytes(m.picture)?,
                complete: m.complete,
                errors: m.errors,
                note: m.note,
                compressed: None,
                text: m.text,
                inner_name: m.inner_name,
                data: bytes(m.data)?.map(Arc::from),
                channel: -1,
                series: String::new(),
                locked: Vec::new(),
                segment_height: 0,
            };
            if let (Some(c), Some(gif)) = (bytes(m.compressed)?, m.compressed_gif) {
                let codec = if gif { Codec::Gif } else { Codec::Jpeg };
                p.compressed = Some((codec, Arc::from(c)));
            }
            if p.picture
                .as_ref()
                .is_some_and(|x| x.len() != p.width as usize * p.height as usize)
            {
                return Err(StateError::Damaged);
            }
            loaded.push(p);
        }
        let n = loaded.len();
        self.products.clear();
        self.jobs.clear();
        for p in loaded.into_iter().rev() {
            self.next_id = self.next_id.max(p.id + 1);
            self.products.push_back(p);
        }
        Ok(n)
    }
}

/// The owner tag of a saved store.
pub const STATE_OWNER: &str = "satimg.products";
/// The version of a saved store. A state of another version is refused.
pub const STATE_VERSION: u32 = 1;

/// A saved store: the products, newest first.
#[derive(serde::Serialize, serde::Deserialize)]
struct SavedStore {
    products: Vec<SavedProduct>,
}

/// A saved product. Bytes are hexadecimal text.
#[derive(serde::Serialize, serde::Deserialize)]
struct SavedProduct {
    id: u32,
    kind: u16,
    name: String,
    vcid: u8,
    apid: u16,
    width: u32,
    height: u32,
    rows: Vec<u32>,
    segments: Vec<u32>,
    segments_total: u32,
    complete: bool,
    errors: bool,
    note: String,
    text: Option<String>,
    inner_name: Option<String>,
    compressed_gif: Option<bool>,
    compressed: Option<String>,
    picture: Option<String>,
    data: Option<String>,
}

/// Why a JPEG 2000 segment produced no picture.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DecodeError {
    /// The decoder refused the code stream.
    Image(ImageError),
    /// The helper thread stopped before it returned a result.
    Stopped,
}

impl std::fmt::Display for DecodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DecodeError::Image(e) => e.fmt(f),
            DecodeError::Stopped => f.write_str("the decoder thread stopped"),
        }
    }
}

impl std::error::Error for DecodeError {}

fn hex(b: &[u8]) -> String {
    let mut s = String::with_capacity(b.len() * 2);
    sdrkit_core::base64::hex_into(b, &mut s);
    s
}

fn unhex(t: &str) -> Option<Vec<u8>> {
    let mut v = Vec::with_capacity(t.len() / 2);
    sdrkit_core::base64::unhex_into(t, &mut v).ok()?;
    Some(v)
}

/// The Rice parameters of a GOES image file: those of its Rice record (record 131) when it has one, else blocks of 16
/// samples with the preprocessor on. One sample interval covers one line, and the basic option set is used (CCSDS 121.0-B-3
/// clause 5.1.2: the restricted set is an option a mission must name).
pub fn rice_params(h: &Headers, img: &ImageStructure) -> Option<rice::Params> {
    let bits = (img.bits as u32).clamp(1, 16);
    match &h.rice {
        Some(r) => r.params(bits as u8, img.columns).ok(),
        None => {
            let mut p =
                rice::Params::new(bits, 16, (img.columns as usize).div_ceil(16).clamp(1, 4096))
                    .ok()?;
            p.preprocess = true;
            Some(p)
        }
    }
}

fn plain_rows(data: &[u8], cols: usize, lines: usize, bits: u32) -> Option<(Vec<Vec<u8>>, bool)> {
    let bpp = if bits > 8 { 2 } else { 1 };
    let shift = bits.saturating_sub(8);
    let mut rows = Vec::new();
    for y in 0..lines {
        let s = y * cols * bpp;
        if s + cols * bpp > data.len() {
            return Some((rows, true));
        }
        let r = &data[s..s + cols * bpp];
        let row = if bpp == 1 {
            r.to_vec()
        } else {
            r.as_chunks::<2>()
                .0
                .iter()
                .map(|c| ((u16::from_be_bytes([c[0], c[1]]) as u32 >> shift).min(255)) as u8)
                .collect()
        };
        rows.push(row);
    }
    Some((rows, false))
}

fn trim_text(b: &[u8]) -> String {
    let end = b.iter().rposition(|&c| c != 0).map_or(0, |i| i + 1);
    String::from_utf8_lossy(&b[..end]).to_string()
}

fn printable(b: &[u8]) -> bool {
    !b.is_empty()
        && b.iter().all(|&c| {
            c == b'\n' || c == b'\r' || c == b'\t' || (32..127).contains(&c) || c >= 0x80 || c == 0
        })
}

fn display_name(name: &str, id: u32, kind: ProductKind) -> String {
    if name.is_empty() {
        format!("{} {id}", kind.label())
    } else {
        name.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sdrkit_proto_ccsds::xrit::headers::{parse, Builder};

    #[test]
    fn names_and_kinds() {
        assert_eq!(gk2a_channel("IMG_FD_143_IR105_20181227_030000_01.lrit"), 12);
        assert_eq!(gk2a_channel("IMG_FD_143_VI006_x"), 2);
        assert_eq!(gk2a_channel("ADD_ANT_143"), -1);
        assert_eq!(GK2A_CHANNELS[0], "VI004");
        assert_eq!(GK2A_CHANNELS[15], "IR133");
        let n = parse_fy4_name("FY4A-_AGRI--_N_DISK_1047E_L1-_FDI-_MULT_NOM_20230101000000_20230101001459_4000M_V0001.HDF").unwrap();
        assert_eq!(
            (
                n.satellite.as_str(),
                n.region.as_str(),
                n.instrument.as_str(),
                n.channel.as_str()
            ),
            ("FY-4A", "DISK", "AGRI", "MULT")
        );
        assert!(parse_fy4_name("something_else.txt").is_none());
        let f = Builder::new(0).image(8, 2712, 226, 1).build(0);
        let h = parse(&f, Dialect::Goes).unwrap();
        assert_eq!(
            classify(&h, "OR_ABI-L2-CMIPF-M6C13_G18"),
            ProductKind::FullDisk
        );
        assert_eq!(
            classify(&h, "OR_ABI-L2-CMIPM1-M6C02"),
            ProductKind::Mesoscale
        );
        let t = parse(&Builder::new(2).build(0), Dialect::Goes).unwrap();
        assert_eq!(classify(&t, "admin"), ProductKind::AdminMessage);
    }
}
