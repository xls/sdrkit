//! The GVAR block format of the GOES I to P imager broadcast: the synchronisation code, the coding of a block, its header,
//! the line documentation of the imager blocks, the block deframer and the reduced pictures.
//!
//! Standard: GOES Operations Ground Equipment Interface Specification DRL 504-02, section 3 (GVAR Transmission Format):
//! 3.2.7 (encoding), 3.3.1 to 3.3.3 (synchronisation code, header, CRC), Tables 3-5, 3-6a and 3-7.

use sdrkit_codec_image::strip::Strip;
use sdrkit_util_bits::crc::catalog::CRC16_GENIBUS;
use sdrkit_util_bits::lfsr::Lfsr;
use sdrkit_util_bits::sync::SyncWord;
use sdrkit_util_bits::BitOrder;
use std::sync::Arc;

/// Bits a second.
pub const BIT_RATE: f64 = 2_111_360.0;
/// Bits of the synchronisation code before each block.
pub const SYNC_BITS: usize = 10_032;
/// Bytes of one copy of the header.
pub const HEADER_BYTES: usize = 30;
/// Bytes of the three copies of the header.
pub const HEADER_FIELD: usize = 3 * HEADER_BYTES;
/// The least bits of a data field (an idle block or a narrow scan).
pub const MIN_DATA_BITS: usize = 21_440;
/// The most bits of a data field (a visible block of a scan 23 degrees wide).
pub const MAX_DATA_BITS: usize = 251_520;
/// The header id of block 0.
pub const BLOCK_0: u8 = 240;
/// The header id of the idle block that the transmitter sends when no block is ready.
pub const IDLE_BLOCK: u8 = 15;
/// Words of the line documentation at the start of each detector record.
pub const LINE_DOC_WORDS: usize = 16;
/// The greatest scan count of an imager frame.
pub const MAX_SCAN: u32 = 1974;
/// The width of the reduced visible picture: every fourth pixel of a scan 19.2 degrees wide.
pub const VISIBLE_WIDTH: usize = 5236;
/// The width of the reduced infrared picture: every second pixel.
pub const INFRARED_WIDTH: usize = 2618;
/// The instrument name of the visible picture.
pub const VISIBLE: &str = "Imager visible";
/// The instrument name of the infrared picture.
pub const INFRARED: &str = "Imager infrared";
/// Wrong bits accepted in the 64 bits that the deframer searches.
const SYNC_ERRORS: u32 = 6;

/// The sequence generator of the synchronisation code and of the block coding: 15 stages, the XOR of stages 8 and 15 enters
/// at stage 1, preset to 51665 octal.
pub fn sequence() -> Lfsr {
    Lfsr::fibonacci(0x8101, 0o51665)
}

/// The last 64 bits of the synchronisation code. The deframer searches these.
pub fn sync_tail() -> u64 {
    let mut s = sequence();
    let mut v = 0u64;
    for i in 0..SYNC_BITS {
        let b = u64::from(s.bit());
        if i >= SYNC_BITS - 64 {
            v = (v << 1) | b;
        }
    }
    v
}

/// The generator in the state after the synchronisation code.
fn after_sync() -> Lfsr {
    let mut s = sequence();
    for _ in 0..SYNC_BITS {
        s.bit();
    }
    s
}

/// Applies the coding of a block to the bytes after the synchronisation code, in place: the sequence of `s` is XORed in, and
/// every second byte is complemented (the first byte after the code is not). `index` is the place of `bytes[0]` in the block.
/// The operation is its own inverse.
pub fn code_bytes(s: &mut Lfsr, index: usize, bytes: &mut [u8]) {
    for (i, b) in bytes.iter_mut().enumerate() {
        *b ^= s.byte(BitOrder::Msb);
        if (index + i) % 2 == 1 {
            *b = !*b;
        }
    }
}

/// The CRC of a header or of a data field: CCITT V.41, preset to ones, sent complemented.
pub fn crc(bytes: &[u8]) -> u16 {
    CRC16_GENIBUS.checksum(bytes)
}

/// The header of a block (Table 3-5).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Header {
    /// The block id as sent: 240 for block 0, 1 to 11, 15 for an idle block.
    pub id: u8,
    /// Bits of a word of the data field: 6, 8 or 10.
    pub word_size: u8,
    /// Words of the data field plus two (the two bytes of the block CRC).
    pub word_count: u16,
    /// What the data field holds.
    pub product: u16,
    /// 1 for new data, 0 for a repeat.
    pub repeat: u8,
    /// The version of the format.
    pub version: u8,
    /// The data valid flag.
    pub data_valid: u8,
    /// The id of the ground processor that made the stream.
    pub sps: u8,
    /// The range word. Its first 4 bits are the spacecraft.
    pub range: u8,
    /// The count of blocks sent, 0 to 65 535. Idle blocks do not raise it.
    pub block_count: u16,
    /// A copy of the header had a good CRC. Without one the fields are a vote over the three copies and are not trusted.
    pub crc_ok: bool,
}

impl Header {
    /// The block number: 0 to 11, or 15 for an idle block. `None` for any other id.
    pub fn block(&self) -> Option<u8> {
        match self.id {
            BLOCK_0 => Some(0),
            1..=11 | IDLE_BLOCK => Some(self.id),
            _ => None,
        }
    }

    /// The spacecraft from the range word: 13 is GOES-N (GOES-13), 14 GOES-O, 15 GOES-P.
    pub fn spacecraft(&self) -> u8 {
        self.range >> 4
    }

    /// Bits of the data field, when the word size and the count give a length that a block can have.
    pub fn data_bits(&self) -> Option<usize> {
        if !matches!(self.word_size, 6 | 8 | 10) || self.word_count < 2 {
            return None;
        }
        let bits = usize::from(self.word_size) * (usize::from(self.word_count) - 2);
        (bits % 8 == 0 && (MIN_DATA_BITS..=MAX_DATA_BITS).contains(&bits)).then_some(bits)
    }

    fn read(c: &[u8], crc_ok: bool) -> Header {
        Header {
            id: c[0],
            word_size: c[1],
            word_count: u16::from_be_bytes([c[2], c[3]]),
            product: u16::from_be_bytes([c[4], c[5]]),
            repeat: c[6],
            version: c[7],
            data_valid: c[8],
            sps: c[10],
            range: c[11],
            block_count: u16::from_be_bytes([c[12], c[13]]),
            crc_ok,
        }
    }

    /// Reads the three decoded copies of a header (90 bytes): the first copy with a good CRC, else a vote bit by bit.
    pub fn parse(copies: &[u8]) -> Header {
        for c in copies[..HEADER_FIELD].as_chunks::<HEADER_BYTES>().0 {
            if crc(&c[..28]) == u16::from_be_bytes([c[28], c[29]]) {
                return Header::read(c, true);
            }
        }
        let mut vote = [0u8; HEADER_BYTES];
        for (i, v) in vote.iter_mut().enumerate() {
            let (a, b, c) = (
                copies[i],
                copies[HEADER_BYTES + i],
                copies[2 * HEADER_BYTES + i],
            );
            *v = (a & b) | (a & c) | (b & c);
        }
        Header::read(&vote, false)
    }

    /// The 30 bytes of one copy, with its CRC.
    pub fn bytes(&self) -> [u8; HEADER_BYTES] {
        let mut c = [0u8; HEADER_BYTES];
        c[0] = self.id;
        c[1] = self.word_size;
        c[2..4].copy_from_slice(&self.word_count.to_be_bytes());
        c[4..6].copy_from_slice(&self.product.to_be_bytes());
        c[6] = self.repeat;
        c[7] = self.version;
        c[8] = self.data_valid;
        c[10] = self.sps;
        c[11] = self.range;
        c[12..14].copy_from_slice(&self.block_count.to_be_bytes());
        let check = crc(&c[..28]);
        c[28..30].copy_from_slice(&check.to_be_bytes());
        c
    }
}

/// Word `k` of 10 bits of a data field, most significant bit first.
pub fn word10(data: &[u8], k: usize) -> u16 {
    let bit = 10 * k;
    let i = bit / 8;
    let v = (u32::from(data[i]) << 16)
        | (u32::from(*data.get(i + 1).unwrap_or(&0)) << 8)
        | u32::from(*data.get(i + 2).unwrap_or(&0));
    ((v >> (14 - bit % 8)) & 0x3FF) as u16
}

/// Packs 10-bit words, most significant bit first, with zero bits up to a whole byte.
pub fn pack10(words: &[u16]) -> Vec<u8> {
    let mut out = vec![0u8; (10 * words.len()).div_ceil(8)];
    for (k, &w) in words.iter().enumerate() {
        for b in 0..10 {
            let bit = 10 * k + b;
            out[bit / 8] |= (((w >> (9 - b)) & 1) as u8) << (7 - bit % 8);
        }
    }
    out
}

/// The line documentation at the start of a detector record (Table 3-7).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LineDoc {
    /// The spacecraft: 13 for GOES-N.
    pub spacecraft: u16,
    /// The detector that is the source of the record.
    pub detector: u16,
    /// The imager channel: 1 visible, 2 (3.9 um), 3 (6.5 um), 4 (10.7 um), 5 (12 um) or 6 (13.3 um).
    pub channel: u16,
    /// The count of output scans since the start of the imager frame, 1 to 1974.
    pub scan: u32,
    /// Pixels of the record.
    pub pixels: u32,
    /// Words of the record, from the first word of the line documentation to the last padding word.
    pub words: u32,
}

impl LineDoc {
    /// Reads the 16 words at word `at` of a data field.
    pub fn read(data: &[u8], at: usize) -> LineDoc {
        let w = |k: usize| word10(data, at + k);
        let pair = |k: usize| (u32::from(w(k)) << 10) | u32::from(w(k + 1));
        LineDoc {
            spacecraft: w(0),
            detector: w(3),
            channel: w(4),
            scan: pair(5),
            pixels: pair(9),
            words: pair(11),
        }
    }

    /// The 16 words.
    pub fn write(&self) -> [u16; LINE_DOC_WORDS] {
        let mut w = [0u16; LINE_DOC_WORDS];
        let split = |v: u32| [((v >> 10) & 0x3FF) as u16, (v & 0x3FF) as u16];
        w[0] = self.spacecraft;
        w[1] = 1;
        w[3] = self.detector;
        w[4] = self.channel;
        w[5..7].copy_from_slice(&split(self.scan));
        w[9..11].copy_from_slice(&split(self.pixels));
        w[11..13].copy_from_slice(&split(self.words));
        w
    }
}

/// A block as sent, from its header on: the three header copies, the data field and its CRC, with the block coding applied.
/// `data` must be a whole number of 16 bits long.
pub fn build_block(header: &Header, data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(HEADER_FIELD + data.len() + 2);
    let copy = header.bytes();
    for _ in 0..3 {
        out.extend_from_slice(&copy);
    }
    out.extend_from_slice(data);
    out.extend_from_slice(&crc(data).to_be_bytes());
    code_bytes(&mut after_sync(), 0, &mut out);
    out
}

/// The bits of the synchronisation code.
pub fn sync_code() -> Vec<u8> {
    let mut s = sequence();
    (0..SYNC_BITS).map(|_| s.bit()).collect()
}

/// A block that the deframer found.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Block {
    /// The header.
    pub header: Header,
    /// The data field. Empty when the header could not be trusted.
    pub data: Vec<u8>,
    /// The CRC of the data field is good.
    pub data_ok: bool,
}

/// Counters of [`Deframer`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DeframerStats {
    /// Synchronisation codes found.
    pub sync_hits: u64,
    /// Blocks with a good header CRC and a known id.
    pub good: u64,
    /// Blocks without a good header copy, with an id that no block has, or with a length that no block can have.
    pub bad: u64,
    /// Idle blocks.
    pub idle: u64,
    /// Blocks whose data field failed its CRC.
    pub data_crc_errors: u64,
}

#[derive(Clone, Copy)]
enum Stage {
    Search,
    Header,
    Data { header: Header, bytes: usize },
}

/// Finds blocks in the decoded bit stream. Blocks have different lengths: the deframer finds the end of each synchronisation
/// code, reads the header behind it, takes the length of the data field from the header, and searches again after the block.
/// A header without a good copy gives no length, so its block is counted as bad and not read.
pub struct Deframer {
    search: SyncWord,
    stage: Stage,
    seq: Lfsr,
    start: Lfsr,
    buf: Vec<u8>,
    acc: u8,
    nbits: u8,
    /// Counters.
    pub stats: DeframerStats,
}

impl Default for Deframer {
    fn default() -> Self {
        let search = SyncWord::new(u128::from(sync_tail()), 64, SYNC_ERRORS)
            .unwrap_or_else(|_| unreachable!("constant sync word"));
        Deframer {
            search,
            stage: Stage::Search,
            seq: after_sync(),
            start: after_sync(),
            buf: Vec::with_capacity(MAX_DATA_BITS / 8 + 2),
            acc: 0,
            nbits: 0,
            stats: DeframerStats::default(),
        }
    }
}

impl Deframer {
    /// A block is being read.
    pub fn in_block(&self) -> bool {
        !matches!(self.stage, Stage::Search)
    }

    /// Searches again from the next bit.
    pub fn reset(&mut self) {
        self.stage = Stage::Search;
        self.search.reset();
        self.buf.clear();
        self.nbits = 0;
    }

    /// Takes decoded bits and appends the blocks they complete to `out`.
    pub fn push(&mut self, bits: &[u8], out: &mut Vec<Block>) {
        for &b in bits {
            if let Stage::Search = self.stage {
                if self.search.push(b).is_some() {
                    self.stats.sync_hits += 1;
                    self.seq = self.start;
                    self.buf.clear();
                    self.nbits = 0;
                    self.stage = Stage::Header;
                }
                continue;
            }
            self.acc = (self.acc << 1) | (b & 1);
            self.nbits += 1;
            if self.nbits < 8 {
                continue;
            }
            self.nbits = 0;
            self.buf.push(self.acc);
            match self.stage {
                Stage::Header if self.buf.len() == HEADER_FIELD => {
                    code_bytes(&mut self.seq, 0, &mut self.buf);
                    let header = Header::parse(&self.buf);
                    self.buf.clear();
                    match (header.crc_ok, header.block(), header.data_bits()) {
                        (true, Some(_), Some(bits)) => {
                            self.stage = Stage::Data {
                                header,
                                bytes: bits / 8 + 2,
                            };
                        }
                        _ => {
                            self.stats.bad += 1;
                            out.push(Block {
                                header,
                                data: Vec::new(),
                                data_ok: false,
                            });
                            self.reset();
                        }
                    }
                }
                Stage::Data { header, bytes } if self.buf.len() == bytes => {
                    code_bytes(&mut self.seq, HEADER_FIELD, &mut self.buf);
                    let n = bytes - 2;
                    let data_ok =
                        crc(&self.buf[..n]) == u16::from_be_bytes([self.buf[n], self.buf[n + 1]]);
                    self.stats.good += 1;
                    self.stats.idle += u64::from(header.id == IDLE_BLOCK);
                    self.stats.data_crc_errors += u64::from(!data_ok);
                    self.buf.truncate(n);
                    out.push(Block {
                        header,
                        data: std::mem::take(&mut self.buf),
                        data_ok,
                    });
                    self.buf.reserve(MAX_DATA_BITS / 8 + 2);
                    self.reset();
                }
                _ => {}
            }
        }
    }
}

/// Counters of [`Imager`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ImagerStats {
    /// The scan count of the last imager block.
    pub scan: u32,
    /// The greatest row of the visible picture plus one.
    pub visible_rows: u32,
    /// The greatest row of the infrared picture plus one.
    pub infrared_rows: u32,
    /// Imager frames whose start flag was seen in block 0.
    pub frame_starts: u64,
    /// Detector records left out: a scan count outside 1 to 1974 or a record that does not fit its block.
    pub records_refused: u64,
}

/// Draws the reduced pictures from the imager blocks. Visible: blocks 3 and 7 (detectors 1 and 5 of the 8), every fourth
/// pixel, row 2 x (scan - 1) and one more for block 7. Infrared: the first detector record of each channel of blocks 1 and 2,
/// every second pixel, row scan - 1; picture channels 1 to 4 are the imager channels 2, 3, 4 and 6 (or 5).
#[derive(Clone, Debug, Default)]
pub struct Imager {
    /// Counters.
    pub stats: ImagerStats,
}

impl Imager {
    /// Starts new pictures.
    pub fn new_picture(&mut self) {
        self.stats.visible_rows = 0;
        self.stats.infrared_rows = 0;
    }

    #[allow(clippy::too_many_arguments)]
    fn strip(
        data: &[u8],
        first: usize,
        doc: &LineDoc,
        step: usize,
        width: usize,
        channel: u16,
        row: u32,
        name: &'static str,
    ) -> Strip {
        let n = (doc.pixels as usize).div_ceil(step).min(width);
        let px: Vec<u8> = (0..n)
            .map(|x| (word10(data, first + step * x) >> 2) as u8)
            .collect();
        let mut s = Strip::new(channel, row, 0, 1, n as u32, Arc::from(px));
        s.instrument = name;
        s
    }

    /// Takes a block with a good header.
    pub fn push(&mut self, block: &Block, out: &mut Vec<Strip>) {
        let Some(id) = block.header.block() else {
            return;
        };
        let data = &block.data[..];
        if id == 0 {
            // Block 0, words 3 to 6: the scan status. Its first bit is 1 in the scan that starts an imager frame.
            self.stats.frame_starts += u64::from(data.len() > 2 && data[2] & 0x80 != 0);
            return;
        }
        if !(1..=10).contains(&id) || block.header.word_size != 10 {
            return;
        }
        let total = data.len() * 8 / 10;
        if total < LINE_DOC_WORDS {
            return;
        }
        let first = LineDoc::read(data, 0);
        let len = first.words as usize;
        if len < LINE_DOC_WORDS + 1 || len > total {
            self.stats.records_refused += 1;
            return;
        }
        let mut seen = [false; 8];
        for r in 0..(total / len).min(if id <= 2 { 4 } else { 1 }) {
            let at = r * len;
            let doc = LineDoc::read(data, at);
            let pixels = doc.pixels as usize;
            if !(1..=MAX_SCAN).contains(&doc.scan) || LINE_DOC_WORDS + pixels > len || pixels == 0 {
                self.stats.records_refused += 1;
                continue;
            }
            self.stats.scan = doc.scan;
            let px = at + LINE_DOC_WORDS;
            if id >= 3 {
                if id != 3 && id != 7 {
                    continue;
                }
                let row = 2 * (doc.scan - 1) + u32::from(id == 7);
                out.push(Self::strip(
                    data,
                    px,
                    &doc,
                    4,
                    VISIBLE_WIDTH,
                    1,
                    row,
                    VISIBLE,
                ));
                self.stats.visible_rows = self.stats.visible_rows.max(row + 1);
            } else {
                let picture_channel = match doc.channel {
                    2 => 1,
                    3 => 2,
                    4 => 3,
                    5 | 6 => 4,
                    _ => {
                        self.stats.records_refused += 1;
                        continue;
                    }
                };
                // The first detector of a channel is drawn. Its second detector is the line between two rows.
                if std::mem::replace(&mut seen[picture_channel as usize], true) {
                    continue;
                }
                let row = doc.scan - 1;
                out.push(Self::strip(
                    data,
                    px,
                    &doc,
                    2,
                    INFRARED_WIDTH,
                    picture_channel,
                    row,
                    INFRARED,
                ));
                self.stats.infrared_rows = self.stats.infrared_rows.max(row + 1);
            }
        }
    }
}

/// The blocks of one scan of the test signal, numbered `scan` (1 to 1974): block 0, the two infrared blocks as GOES-N sends
/// them (channels 2 and 3 with two detectors each; channel 4 with two detectors and channel 6 with one), the eight visible
/// blocks and block 11. Infrared pixel `x` of picture channel `c` (0 to 3) is `value(c, x, scan)`. Visible pixel `x` of block
/// `b` is `value(4, x, 8 scan + b - 3)`. `count` is the block count of the first block. Each entry is the header and the data
/// field.
pub fn test_scan(
    scan: u32,
    count: u16,
    value: impl Fn(u16, u32, u32) -> u16,
) -> Vec<(Header, Vec<u8>)> {
    const SPACECRAFT: u8 = 13;
    let header = |id: u8, word_size: u8, words: usize, product: u16, k: u16| Header {
        id,
        word_size,
        word_count: (words + 2) as u16,
        product,
        repeat: 1,
        version: 2,
        data_valid: 1,
        sps: 1,
        range: SPACECRAFT << 4,
        block_count: count.wrapping_add(k),
        crc_ok: true,
    };
    let record = |words: &mut Vec<u16>, len: usize, detector: u16, channel: u16, px: &[u16]| {
        let doc = LineDoc {
            spacecraft: u16::from(SPACECRAFT),
            detector,
            channel,
            scan,
            pixels: px.len() as u32,
            words: len as u32,
        };
        let start = words.len();
        words.extend_from_slice(&doc.write());
        words.extend_from_slice(px);
        words.resize(start + len, 0);
    };
    let mut blocks = Vec::with_capacity(12);
    let mut zero = vec![0u8; 8040];
    zero[2] = if scan == 1 { 0x80 } else { 0 };
    blocks.push((header(BLOCK_0, 8, 8040, 3, 0), zero));
    // Infrared: 5236 pixels a record for a scan 19.2 degrees wide.
    let ir = |c: u16| -> Vec<u16> { (0..5236).map(|x| value(c, x, scan)).collect() };
    let mut words = Vec::new();
    for (detector, channel, c) in [(1, 2, 0), (2, 2, 0), (1, 3, 1), (2, 3, 1)] {
        record(&mut words, 5252, detector, channel, &ir(c));
    }
    blocks.push((header(1, 10, words.len(), 4, 1), pack10(&words)));
    // Three records: the record length makes the data field a whole number of 16 bits.
    let mut words = Vec::new();
    for (detector, channel, c) in [(1, 4, 2), (2, 4, 2), (1, 6, 3)] {
        record(&mut words, 5256, detector, channel, &ir(c));
    }
    blocks.push((header(2, 10, words.len(), 4, 2), pack10(&words)));
    for b in 3..=10u8 {
        let px: Vec<u16> = (0..20_944)
            .map(|x| value(4, x, 8 * scan + u32::from(b) - 3))
            .collect();
        let mut words = Vec::new();
        record(&mut words, 20_960, u16::from(b) - 2, 1, &px);
        blocks.push((header(b, 10, words.len(), 5, u16::from(b)), pack10(&words)));
    }
    blocks.push((header(11, 8, 8040, 0, 11), vec![0u8; 8040]));
    blocks
}
