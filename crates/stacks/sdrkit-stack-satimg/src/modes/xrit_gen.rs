//! The test signals of the xRIT stacks: the GOES test products, the GK-2A channel pictures in segments (one channel
//! DES-encrypted with the test key when requested), and Elektro-L (the GOES products at the rate of the link, or GGAK frames).

use super::xrit::{
    ElektroLink, RsBasis, XritMode, GEN_BASIS, GEN_COUNTER, GEN_ENCRYPT, GEN_FILL, GEN_INVERT_G2,
    GEN_LINK, GEN_NRZM, GEN_ONCE,
};
use crate::gen::{
    aos_frame, cadu_bytes, space_packet, test_pixel, BitSource, CcsdsCoder, CodeOptions, GenError,
    ModParams, Modulator, ZoneStream,
};
use crate::psk::Modulation;
use crate::xrit::crypto::{des_ecb_encrypt, TEST_KEY};
use sdrkit_algo_fec::rs::Basis;
use sdrkit_core::{Complex32, ConfigError, GenConfig, Generator, SignalInfo, Values};
use sdrkit_proto_ccsds::aos::Layout;
use sdrkit_proto_ccsds::rice;
use sdrkit_proto_ccsds::tm::FirstHeader;
use sdrkit_proto_ccsds::xrit::headers::{Builder, RiceRecord};
use sdrkit_proto_ccsds::xrit::transport::split_file;
use std::collections::{BTreeMap, VecDeque};

/// One file to send: virtual channel, APID, bytes, and the offsets where a packet must start (Rice lines).
#[derive(Clone, Debug, PartialEq)]
pub struct TestFile {
    /// Virtual channel.
    pub vcid: u8,
    /// APID.
    pub apid: u16,
    /// The file.
    pub bytes: Vec<u8>,
    /// Offsets where a packet must start.
    pub line_starts: Vec<usize>,
}

/// The test time stamp: 2026-11-01 03:00:00 UTC.
pub const TEST_TIME: f64 = 1_793_502_000.0;

fn noaa(product: u16, sub: u16, param: u16, comp: u8) -> Vec<u8> {
    let mut b = b"NOAA".to_vec();
    b.extend_from_slice(&product.to_be_bytes());
    b.extend_from_slice(&sub.to_be_bytes());
    b.extend_from_slice(&param.to_be_bytes());
    b.push(comp);
    b
}

/// The GOES full-disk test picture (100 x 64).
pub fn goes_full_disk() -> Vec<u8> {
    (0..100 * 64)
        .map(|i| test_pixel(13, (i % 100) as u32, (i / 100) as u32, 100))
        .collect()
}

/// The GOES mesoscale test picture (60 x 40).
pub fn goes_mesoscale() -> Vec<u8> {
    (0..60 * 40)
        .map(|i| test_pixel(2, (i % 60) as u32, (i / 60) as u32, 60))
        .collect()
}

/// The relayed JPEG test picture (64 x 48), as its JFIF bytes.
pub fn goes_relay_jpeg() -> Vec<u8> {
    let px: Vec<u8> = (0..64 * 48)
        .map(|i| test_pixel(1, (i % 64) as u32, (i / 64) as u32, 64))
        .collect();
    sdrkit_codec_image::Raster::from_vec(sdrkit_codec_image::PixelFormat::Grey8, 64, 48, px)
        .and_then(|r| {
            sdrkit_codec_image::jpeg::encode(&r, 75, sdrkit_codec_image::jpeg::Subsampling::S444)
        })
        .unwrap_or_default()
}

/// The administrative message.
pub const ADMIN_TEXT: &str = "HRIT TEST SIGNAL\r\nADMINISTRATIVE MESSAGE\r\nThis is a synthetic administrative message for receiver tests. Nothing is transmitted.\r\n";
/// The EMWIN text.
pub const EMWIN_TEXT: &str =
    "EMWIN TEST PRODUCT\r\nA synthetic text product in a stored ZIP archive.\r\n";

#[allow(clippy::too_many_arguments)]
/// The GOES full-disk segment of `lines` lines starting at `first`, Rice coded with blocks of 16 samples of `bits` bits.
pub fn goes_rice_segment(
    name: &str,
    pic: &[u8],
    width: u16,
    first: u16,
    lines: u16,
    total: u16,
    seq: u16,
    bits: u8,
) -> TestFile {
    let rec = RiceRecord {
        flags: RiceRecord::ALLOW_K13 | RiceRecord::MSB | RiceRecord::NN,
        pixels_per_block: 16,
        lines_per_packet: 1,
    };
    let mut seg_rec = Vec::new();
    for v in [0x1267u16, seq, 0, first, total, width, lines * total] {
        seg_rec.extend_from_slice(&v.to_be_bytes());
    }
    let b = Builder::new(0)
        .image(bits, width, lines, 1)
        .navigation(
            "GEOS(-75.0)",
            [20_425_338.0, 20_425_338.0, 50.0, 32.0 - first as f64],
            false,
        )
        .annotation(name)
        .time_unix(TEST_TIME)
        .record(128, &seg_rec)
        .record(129, &noaa(16, 1, 13, 1))
        .rice(&rec);
    let p = rec.params(bits, width).ok();
    let mut data = Vec::new();
    let mut starts = Vec::new();
    let shift = 8u8.saturating_sub(bits);
    for y in 0..lines as usize {
        starts.push(data.len());
        let row: Vec<u32> = pic
            [(first as usize + y) * width as usize..(first as usize + y + 1) * width as usize]
            .iter()
            .map(|&v| (v >> shift) as u32)
            .collect();
        if let Some(p) = &p {
            let _ = rice::encode(&row, p, &mut data);
        }
    }
    let head = b.build(data.len());
    let hl = head.len();
    let mut bytes = head;
    bytes.extend_from_slice(&data);
    TestFile {
        vcid: 13,
        apid: 781,
        bytes,
        line_starts: starts.iter().map(|s| s + hl).filter(|&s| s != hl).collect(),
    }
}

/// The name of the full-disk test product.
pub const FULL_DISK_NAME: &str =
    "OR_ABI-L2-CMIPF-M6C13_G16_s20263050300000_e20263050309590_c20263050310000.lrit";
/// The name of the mesoscale test product.
pub const MESOSCALE_NAME: &str =
    "OR_ABI-L2-CMIPM1-M6C02_G16_s20263050301000_e20263050301059_c20263050301100.lrit";

/// The five GOES test products (the full disk in two Rice segments, the mesoscale picture plain, a relayed JPEG, an
/// administrative text and an EMWIN ZIP).
pub fn goes_files() -> Vec<TestFile> {
    let fd = goes_full_disk();
    let mut files = vec![
        goes_rice_segment(FULL_DISK_NAME, &fd, 100, 0, 32, 2, 1, 8),
        goes_rice_segment(FULL_DISK_NAME, &fd, 100, 32, 32, 2, 2, 8),
    ];
    let meso = goes_mesoscale();
    let b = Builder::new(0)
        .image(8, 60, 40, 0)
        .annotation(MESOSCALE_NAME)
        .time_unix(TEST_TIME + 60.0)
        .record(129, &noaa(16, 3, 2, 0));
    let mut bytes = b.build(meso.len());
    let hl = bytes.len();
    bytes.extend_from_slice(&meso);
    // One line a packet.
    files.push(TestFile {
        vcid: 2,
        apid: 514,
        bytes,
        line_starts: (1..40).map(|y| hl + y * 60).collect(),
    });
    let jpeg = goes_relay_jpeg();
    let b = Builder::new(0)
        .image(8, 64, 48, 2)
        .annotation("relay_test_picture.jpg")
        .time_unix(TEST_TIME)
        .record(129, &noaa(43, 0, 0, 2));
    let mut bytes = b.build(jpeg.len());
    bytes.extend_from_slice(&jpeg);
    files.push(TestFile {
        vcid: 30,
        apid: 1054,
        bytes,
        line_starts: vec![],
    });
    let b = Builder::new(2)
        .annotation("admin_test_message")
        .time_unix(TEST_TIME)
        .record(129, &noaa(1, 0, 0, 0));
    let mut bytes = b.build(ADMIN_TEXT.len());
    bytes.extend_from_slice(ADMIN_TEXT.as_bytes());
    files.push(TestFile {
        vcid: 0,
        apid: 1,
        bytes,
        line_starts: vec![],
    });
    let zip = crate::xrit::zip::stored("TESTEMWN.TXT", EMWIN_TEXT.as_bytes());
    let b = Builder::new(214)
        .annotation("Z_EMWIN_TEST.ZIP")
        .time_unix(TEST_TIME)
        .record(129, &noaa(9, 0, 0, 10));
    let mut bytes = b.build(zip.len());
    bytes.extend_from_slice(&zip);
    files.push(TestFile {
        vcid: 20,
        apid: 1051,
        bytes,
        line_starts: vec![],
    });
    files
}

/// The GK-2A test channel pictures: 128 x 48, 10 bits.
pub fn gk2a_picture(channel: &str) -> Vec<u16> {
    let ch = if channel == "IR105" { 12 } else { 2 };
    (0..128 * 48)
        .map(|i| {
            ((test_pixel(ch, (i % 128) as u32, (i / 128) as u32, 128) as u16) << 2) | (i as u16 & 3)
        })
        .collect()
}

/// The schedule text of the GK-2A test signal.
pub const GK2A_SCHEDULE: &str = "GK-2A TEST SIGNAL OBSERVATION SCHEDULE\r\n0300 FD\r\n0310 FD\r\n";

/// The GK-2A test files: IR105 and VI006 in 4 segments each (10-bit samples, 2 bytes each, no compression), a schedule text and
/// an additional-data file. With `encrypt_vi006` the VI006 segments are DES-encrypted with key 1 ([`TEST_KEY`]).
pub fn gk2a_files(encrypt_vi006: bool) -> Vec<TestFile> {
    let mut files = Vec::new();
    for (ch, apid) in [("IR105", 12u16), ("VI006", 2u16)] {
        let pic = gk2a_picture(ch);
        for seg in 0..4u16 {
            let rows = &pic[seg as usize * 12 * 128..(seg as usize + 1) * 12 * 128];
            let mut data: Vec<u8> = rows.iter().flat_map(|v| v.to_be_bytes()).collect();
            let key = if encrypt_vi006 && ch == "VI006" {
                1u32
            } else {
                0
            };
            if key != 0 {
                des_ecb_encrypt(&TEST_KEY, &mut data);
            }
            let name = format!("IMG_FD_001_{ch}_20261101_030000_{:02}.lrit", seg + 1);
            let first = seg * 12;
            let b = Builder::new(0)
                .image(10, 128, 12, 0)
                .navigation(
                    "GEOS(128.2)",
                    [40_932_549.0, 40_932_549.0, 64.0, 48.0],
                    false,
                )
                .annotation(&name)
                .time_unix(TEST_TIME)
                .record(7, &key.to_be_bytes())
                .record(128, &[seg as u8 + 1, 4, (first >> 8) as u8, first as u8])
                .record(131, b"2026-11-01 03:00:00");
            let mut bytes = b.build(data.len());
            bytes.extend_from_slice(&data);
            files.push(TestFile {
                vcid: 0,
                apid,
                bytes,
                line_starts: vec![],
            });
        }
    }
    let b = Builder::new(2)
        .annotation("FD_SCHEDULE_20261101.txt")
        .time_unix(TEST_TIME);
    let mut bytes = b.build(GK2A_SCHEDULE.len());
    bytes.extend_from_slice(GK2A_SCHEDULE.as_bytes());
    files.push(TestFile {
        vcid: 4,
        apid: 128,
        bytes,
        line_starts: vec![],
    });
    let add: Vec<u8> = (0..300).map(|i| (i * 7) as u8).collect();
    let b = Builder::new(255)
        .annotation("ADD_ANT_143_20261101.bin")
        .time_unix(TEST_TIME);
    let mut bytes = b.build(add.len());
    bytes.extend_from_slice(&add);
    files.push(TestFile {
        vcid: 5,
        apid: 160,
        bytes,
        line_starts: vec![],
    });
    files
}

/// The transfer frames that carry `files` (then `fill` fill frames): frames of spacecraft `sc`, packets with the CRC, each
/// file on its own virtual channel, the zones closed with an idle packet.
pub struct XritFrames {
    sc: u8,
    files: Vec<TestFile>,
    fill: usize,
    once: bool,
    counters: BTreeMap<u8, u32>,
    queue: VecDeque<Vec<u8>>,
    file_counter: u16,
    seq: BTreeMap<u16, u16>,
    rounds: u32,
    layout: Layout,
}

impl XritFrames {
    /// Frames for `files`, then `fill` fill frames, repeated (or once).
    pub fn new(sc: u8, files: Vec<TestFile>, fill: usize, once: bool) -> Self {
        XritFrames {
            sc,
            files,
            fill,
            once,
            counters: BTreeMap::new(),
            queue: VecDeque::new(),
            file_counter: 0,
            seq: BTreeMap::new(),
            rounds: 0,
            layout: Layout::RS_I4,
        }
    }
    /// Starts the frame count of every virtual channel at `n`.
    pub fn with_counter_start(mut self, n: u32) -> Self {
        for vc in 0..64u8 {
            self.counters.insert(vc, n & 0xFF_FFFF);
        }
        self
    }
    /// Rounds queued so far.
    pub fn rounds(&self) -> u32 {
        self.rounds
    }

    fn counter(&mut self, vc: u8) -> u32 {
        let c = self.counters.entry(vc).or_insert(0);
        let v = *c;
        *c = (*c + 1) & 0xFF_FFFF;
        v
    }

    fn fill_frame(&mut self) -> Vec<u8> {
        let c = self.counter(63);
        aos_frame(&self.layout, self.sc, 63, c, FirstHeader::None, &[])
    }

    fn queue_round(&mut self) {
        if self.once && self.rounds > 0 {
            for _ in 0..64 {
                let f = self.fill_frame();
                self.queue.push_back(f);
            }
            return;
        }
        self.rounds += 1;
        let zl = self.layout.data_zone_len();
        for f in self.files.clone() {
            let pieces = split_file(&f.bytes, self.file_counter, 8000, &f.line_starts);
            self.file_counter = self.file_counter.wrapping_add(1);
            let mut zs = ZoneStream::default();
            let n = pieces.len();
            for (k, user) in pieces.iter().enumerate() {
                let flags = match (k == 0, k + 1 == n) {
                    (true, true) => 3,
                    (true, false) => 1,
                    (false, true) => 2,
                    _ => 0,
                };
                let s = self.seq.entry(f.apid).or_insert(0);
                let p = space_packet(f.apid, flags, *s, user, true);
                *s = (*s + 1) & 0x3FFF;
                zs.push(&p);
            }
            // Close the last zone with an idle packet.
            let r = zs.len() % zl;
            if r != 0 {
                let mut fill = zl - r;
                if fill < 7 {
                    fill += zl;
                }
                zs.push(&space_packet(2047, 3, 0, &vec![0u8; fill - 6], false));
            }
            while !zs.is_empty() {
                let (fhp, zone) = zs.take(zl);
                let c = self.counter(f.vcid);
                self.queue
                    .push_back(aos_frame(&self.layout, self.sc, f.vcid, c, fhp, &zone));
            }
        }
        for _ in 0..self.fill {
            let f = self.fill_frame();
            self.queue.push_back(f);
        }
    }

    /// The next transfer frame (892 bytes).
    pub fn next_frame(&mut self) -> Vec<u8> {
        loop {
            if let Some(f) = self.queue.pop_front() {
                return f;
            }
            self.queue_round();
        }
    }
}

/// The options of an xRIT generator.
#[derive(Clone, Debug, PartialEq)]
pub struct XritGenParams {
    /// The modulator.
    pub modem: ModParams,
    /// G2 inverted.
    pub invert_g2: bool,
    /// NRZ-M before the code.
    pub nrzm: bool,
    /// The Reed-Solomon basis.
    pub basis: Basis,
    /// Fill frames after each round.
    pub fill: usize,
    /// The files once only.
    pub once: bool,
    /// The first frame count.
    pub counter_start: u32,
}

impl XritGenParams {
    /// The options of a generator configuration: `gen_invert_g2`, `gen_nrzm`, `gen_basis`, `gen_fill`, `gen_once` and
    /// `gen_counter`. The schema of `v` must be the stack's.
    pub fn from_values(
        rate: f64,
        rs: f64,
        modulation: Modulation,
        zero_is_plus: bool,
        v: &Values,
    ) -> Self {
        let mut modem = ModParams::new(rate, rs, 0.5, modulation);
        modem.zero_is_plus = zero_is_plus;
        XritGenParams {
            modem,
            invert_g2: v.get(&GEN_INVERT_G2),
            nrzm: v.get(&GEN_NRZM),
            basis: match v.get(&GEN_BASIS) {
                RsBasis::Conventional => Basis::Conventional,
                RsBasis::Dual => Basis::Dual,
            },
            fill: v.get(&GEN_FILL).clamp(0, 100_000) as usize,
            once: v.get(&GEN_ONCE),
            counter_start: v.get(&GEN_COUNTER).clamp(0, 0xFF_FFFF) as u32,
        }
    }
}

/// The channel bits of an xRIT link.
pub struct XritSource {
    frames: XritFrames,
    coder: CcsdsCoder,
    basis: Basis,
}

impl BitSource for XritSource {
    fn next_bits(&mut self, out: &mut Vec<u8>) {
        let v = self.frames.next_frame();
        let cadu = cadu_bytes(&v, self.basis, true);
        self.coder.push(&cadu, out);
    }
}

impl XritSource {
    /// The source of `files` sent by spacecraft `sc` with the options `p`.
    pub fn new(p: &XritGenParams, sc: u8, files: Vec<TestFile>) -> Self {
        XritSource {
            frames: XritFrames::new(sc, files, p.fill, p.once).with_counter_start(p.counter_start),
            coder: CcsdsCoder::new(CodeOptions {
                nrzm: p.nrzm,
                invert_g2: p.invert_g2,
                conv: true,
            }),
            basis: p.basis,
        }
    }
    /// The frames.
    pub fn frames(&self) -> &XritFrames {
        &self.frames
    }
}

/// GGAK channel bits: 224-byte frames, each sent twice, the marker and the sub-frame counting, a time stamp and 13 constant
/// field triples.
pub struct GgakSource {
    n: u64,
}

/// The field the GGAK test signal sends.
pub const GGAK_FIELD: [i16; 3] = [12_000, -8_000, 30_000];

impl BitSource for GgakSource {
    fn next_bits(&mut self, out: &mut Vec<u8>) {
        let k = self.n;
        self.n += 1;
        let f = crate::instruments::ggak::build(
            ((k / 16) % 16) as u8,
            (k % 16) as u8,
            1_000_000 + k,
            &[GGAK_FIELD; 13],
        );
        for _ in 0..2 {
            for &b in &f {
                for i in (0..8).rev() {
                    out.push((b >> i) & 1);
                }
            }
        }
    }
}

/// The field triple of the Arktika-M GGAK test signal.
pub const ARKTIKA_FIELD: [i16; 3] = [12_000, -3_400, 30_000];

/// The Arktika-M GGAK test frame `k`: marker `k mod 8`, sub-frame 7 with marker 7 and 0 otherwise, time stamp `k`, the
/// triple [`ARKTIKA_FIELD`] thirteen times, bytes AA up to the CRC.
pub fn arktika_frame(k: u64) -> Vec<u8> {
    let marker = (k % 8) as u8;
    let mut f = crate::instruments::ggak::build(
        marker,
        if marker == 7 { 7 } else { 0 },
        k,
        &[ARKTIKA_FIELD; 13],
    );
    f[13 + 13 * 6..222].fill(0xAA);
    let c = crate::instruments::ggak::crc(&f);
    f[222..224].copy_from_slice(&c.to_be_bytes());
    f
}

/// Arktika-M GGAK channel bits: the frames of [`arktika_frame`], each sent twice.
#[derive(Default)]
pub struct ArktikaSource {
    n: u64,
}

impl BitSource for ArktikaSource {
    fn next_bits(&mut self, out: &mut Vec<u8>) {
        let f = arktika_frame(self.n);
        self.n += 1;
        for _ in 0..2 {
            for &b in &f {
                out.extend((0..8).rev().map(|i| (b >> i) & 1));
            }
        }
    }
}

/// An xRIT test signal: the clean signal at unit mean power.
pub struct XritGenerator<S: BitSource> {
    rate: f64,
    info: SignalInfo,
    m: Modulator<S>,
}

impl<S: BitSource> XritGenerator<S> {
    /// The signal of `source` with the modulator options `modem`.
    ///
    /// # Errors
    ///
    /// [`GenError`] if the modulator refuses the options.
    pub fn new(modem: ModParams, source: S) -> Result<Self, GenError> {
        Ok(XritGenerator {
            rate: modem.rate,
            info: modem.signal_info(),
            m: Modulator::new(modem, source)?,
        })
    }
    /// The bit source.
    pub fn source_mut(&mut self) -> &mut S {
        self.m.source_mut()
    }
}

impl<S: BitSource> Generator for XritGenerator<S> {
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

fn make(cfg: &GenConfig, mode: XritMode) -> Result<Box<dyn Generator>, GenError> {
    let v = &cfg.settings;
    match mode {
        XritMode::Elektro if cfg.submode.as_deref() == Some(super::xrit::ARKTIKA_SUBMODE) => {
            let modem = ModParams::new(cfg.rate_hz, 5000.0, 0.5, Modulation::Bpsk);
            Ok(Box::new(XritGenerator::new(
                modem,
                ArktikaSource::default(),
            )?))
        }
        XritMode::Goes | XritMode::Elektro => {
            let mut rs = mode.link().symbol_rate;
            if mode == XritMode::Elektro {
                let link = v.get(&GEN_LINK);
                if link == ElektroLink::Ggak {
                    let modem = ModParams::new(cfg.rate_hz, 5000.0, 0.5, Modulation::Bpsk);
                    return Ok(Box::new(XritGenerator::new(modem, GgakSource { n: 0 })?));
                }
                rs = link.link().symbol_rate;
            }
            let p = XritGenParams::from_values(cfg.rate_hz, rs, Modulation::Bpsk, true, v);
            let src = XritSource::new(&p, 126, goes_files());
            Ok(Box::new(XritGenerator::new(p.modem, src)?))
        }
        XritMode::Gk2aLrit | XritMode::Gk2aHrit => {
            let (rs, m, zp) = if mode == XritMode::Gk2aHrit {
                (3e6, Modulation::Qpsk, false)
            } else {
                (128e3, Modulation::Bpsk, true)
            };
            let p = XritGenParams::from_values(cfg.rate_hz, rs, m, zp, v);
            let src = XritSource::new(&p, 0xBA, gk2a_files(v.get(&GEN_ENCRYPT)));
            Ok(Box::new(XritGenerator::new(p.modem, src)?))
        }
    }
}

/// The generator of an xRIT stack. If the modulator refuses its options, it returns the error.
pub fn generator(cfg: &GenConfig, mode: XritMode) -> Result<Box<dyn Generator>, ConfigError> {
    make(cfg, mode).map_err(super::gen_error)
}
