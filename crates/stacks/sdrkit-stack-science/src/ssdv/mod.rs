//! SSDV pictures (`ssdv`): 256-byte packets that carry a JPEG picture, received as 2-FSK (plain bit streams or 8N2 bytes),
//! checked with RS(255,223) and CRC-32, and assembled into pictures. The graph is an FM front end, a bank of bit slicers and
//! packet readers, and the picture assembler, whose pictures leave on the picture stream.

use crate::calendar;
use crate::family::{lock_state_of, lowpass, status, wall, REPORT_S};
use sdrkit_algo_dsp::decim::FirDecimator;
use sdrkit_algo_dsp::fm::Discriminator;
use sdrkit_algo_dsp::nco::Nco;
use sdrkit_algo_fec::rs::{codes::CCSDS_255_223_CONVENTIONAL, Rs};
use sdrkit_algo_sim::modulate::Cpm;
use sdrkit_codec_image::jpeg::{
    decode_block, encode_block, rgb_to_mcus_420, write_mcus_420, EntropyReader, EntropyWriter,
    Mcu420, StandardTables,
};
use sdrkit_core::media::{Codec, Picture, PictureIds, PictureState};
use crate::typed::{SsdvFields, SsdvPacketRow, SsdvPictureRow, SSDV_PACKETS, SSDV_PICTURES};
use sdrkit_core::interface::{Fields, Time};
use sdrkit_core::publish::{field, Published};
use sdrkit_core::verify::{CheckLine, VerifyCase, VerifyHooks, VerifyOptions};
use sdrkit_core::{
    Choice, Clock, CommandError, CommandId, CommandSet, Commands, Complex32, ConfigError,
    GenConfig, Generator, Hz, Receiver, Record, Results, ResultsDecl, Setting, SettingError,
    SettingId, SettingSet, SignalInfo, StackConfig, StateBlob, StateError, Status, Value, Values,
};
use sdrkit_graph::{
    values_of, Applied, Catalog, Component, ComponentInfo, Format, GraphBuilder, GraphError, Io,
    Item, Kind, Port, RateFrom, RunContext, StackGraph, Timed,
};
use sdrkit_util_bits::crc::catalog::CRC32_ISO_HDLC;
use sdrkit_util_objects::commands as objcmd;
use sdrkit_util_objects::shaped::{log_rows, RowOut, Shaped};
use sdrkit_util_objects::{Format as StateFormat, LogRing, Seen, Shift, Tables};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::sync::Arc;

/// The JPEG quality of an SSDV quality level 0 to 7.
pub fn quality_of_level(level: u8) -> u32 {
    [12, 25, 37, 50, 62, 75, 87, 100][level.min(7) as usize]
}

/// Encodes one MCU (no byte stuffing). `dc` holds the predictors of Y, Cb and Cr.
fn encode_mcu(w: &mut EntropyWriter, m: &Mcu420, dc: &mut [i32; 3]) {
    let t = StandardTables::get();
    for (b, blk) in m.iter().enumerate() {
        let c = b.saturating_sub(3);
        let (d, a) = if c == 0 {
            (&t.dc_luma, &t.ac_luma)
        } else {
            (&t.dc_chroma, &t.ac_chroma)
        };
        // The coefficients come from the baseline quantiser, so they always fit their categories.
        let _ = encode_block(w, d, a, &mut dc[c], blk);
    }
}

/// Decodes the MCU that starts at the first byte of `data`. Returns the MCU and its length in whole bytes (the next MCU starts on
/// the next byte), or `None` when the data ends or is not a valid MCU. The length comes from coding the MCU again with the same
/// predictors: the Annex K codes are canonical, so the length is the same.
fn decode_mcu(data: &[u8], dc: &mut [i32; 3], scratch: &mut Vec<u8>) -> Option<(Mcu420, usize)> {
    let t = StandardTables::get();
    let start = *dc;
    let mut r = EntropyReader::new(data, false);
    let mut m: Mcu420 = [[0; 64]; 6];
    for (b, blk) in m.iter_mut().enumerate() {
        let c = b.saturating_sub(3);
        let (d, a) = if c == 0 {
            (&t.dc_luma, &t.ac_luma)
        } else {
            (&t.dc_chroma, &t.ac_chroma)
        };
        decode_block(&mut r, d, a, &mut dc[c], blk).ok()?;
    }
    if r.overrun() {
        return None;
    }
    scratch.clear();
    let mut w = EntropyWriter::new(scratch, false);
    let mut p = start;
    encode_mcu(&mut w, &m, &mut p);
    let len = w.bit_len().div_ceil(8);
    (len <= data.len()).then_some((m, len))
}

/// Packet length.
pub const PACKET: usize = 256;
/// Payload of a normal packet (with Reed–Solomon).
pub const PAYLOAD_FEC: usize = 205;
/// Payload of a packet without FEC.
pub const PAYLOAD_NOFEC: usize = 237;
const HDR: usize = 15;

/// Encodes a call sign (up to 6 characters) in base 40.
pub fn encode_call(call: &str) -> u32 {
    let mut x = 0u32;
    for c in call.chars().take(6).collect::<Vec<_>>().iter().rev() {
        let c = c.to_ascii_uppercase();
        let v = match c {
            '0'..='9' => c as u32 - '0' as u32 + 1,
            'A'..='Z' => c as u32 - 'A' as u32 + 14,
            _ => 0,
        };
        x = x * 40 + v;
    }
    x
}

/// Decodes a base-40 call sign.
pub fn decode_call(mut x: u32) -> String {
    let mut s = String::new();
    while x > 0 {
        let v = x % 40;
        x /= 40;
        s.push(match v {
            1..=10 => (b'0' + v as u8 - 1) as char,
            14..=39 => (b'A' + v as u8 - 14) as char,
            _ => '-',
        });
    }
    s
}

/// The header of a packet.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Header {
    /// Normal (with RS) or no-FEC.
    pub fec: bool,
    /// Call sign.
    pub call: String,
    /// Image id.
    pub image: u8,
    /// Packet id.
    pub packet: u16,
    /// Width in pixels.
    pub width: u16,
    /// Height.
    pub height: u16,
    /// Quality level 0–7.
    pub quality: u8,
    /// End of the image.
    pub eoi: bool,
    /// Offset of the first MCU starting in this packet (0xFF none).
    pub mcu_offset: u8,
    /// Index of that MCU.
    pub mcu_index: u16,
}

/// Parses a checked packet's header.
pub fn header(p: &[u8; PACKET]) -> Header {
    Header {
        fec: p[1] == 0x66,
        call: decode_call(u32::from_be_bytes([p[2], p[3], p[4], p[5]])),
        image: p[6],
        packet: u16::from_be_bytes([p[7], p[8]]),
        width: p[9] as u16 * 16,
        height: p[10] as u16 * 16,
        quality: p[11] & 7,
        eoi: p[11] & 8 != 0,
        mcu_offset: p[12],
        mcu_index: u16::from_be_bytes([p[13], p[14]]),
    }
}

/// The payload of a packet.
pub fn payload(p: &[u8; PACKET]) -> &[u8] {
    if p[1] == 0x66 {
        &p[HDR..HDR + PAYLOAD_FEC]
    } else {
        &p[HDR..HDR + PAYLOAD_NOFEC]
    }
}

/// Why a packet was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CheckError {
    /// Unknown packet type.
    Type,
    /// Beyond the Reed–Solomon code.
    Rs,
    /// CRC-32 mismatch.
    Crc,
}

/// The packet check: the Reed-Solomon decoder of the normal packet type and the CRC-32.
pub struct PacketCheck {
    rs: Rs,
}

impl PacketCheck {
    /// A check with its decoder (`sdrkit_algo_fec::rs::codes::CCSDS_255_223_CONVENTIONAL`).
    pub fn new() -> PacketCheck {
        PacketCheck {
            rs: Rs::new(CCSDS_255_223_CONVENTIONAL).expect("SSDV code"),
        }
    }

    /// Checks and corrects a packet. The sync byte becomes 0x55. The normal type is corrected with RS(255,223), then the
    /// CRC-32 decides. Returns the bytes corrected. If the check fails, the packet keeps its received bytes and the error says
    /// why.
    pub fn check(&mut self, p: &mut [u8; PACKET]) -> Result<usize, CheckError> {
        p[0] = 0x55;
        match p[1] {
            0x67 => {
                let end = HDR + PAYLOAD_NOFEC;
                let c = u32::from_be_bytes([p[end], p[end + 1], p[end + 2], p[end + 3]]);
                if CRC32_ISO_HDLC.checksum(&p[1..end]) != c {
                    return Err(CheckError::Crc);
                }
                Ok(0)
            }
            _ => {
                let mut w = [0u8; PACKET - 1];
                w.copy_from_slice(&p[1..]);
                let n = self.rs.decode(&mut w, &[]).map_err(|_| CheckError::Rs)?;
                if w[0] != 0x66 {
                    return Err(CheckError::Type);
                }
                let end = HDR + PAYLOAD_FEC;
                let c = u32::from_be_bytes([w[end - 1], w[end], w[end + 1], w[end + 2]]);
                if CRC32_ISO_HDLC.checksum(&w[..end - 1]) != c {
                    return Err(CheckError::Crc);
                }
                p[1..].copy_from_slice(&w);
                Ok(n.corrected as usize)
            }
        }
    }
}

impl Default for PacketCheck {
    fn default() -> Self {
        PacketCheck::new()
    }
}

/// Cuts a picture's MCUs into SSDV packets. Every MCU starts on a byte; the DC predictors restart at the first MCU that starts in
/// each packet, so a packet decodes on its own.
pub fn make_packets(
    call: &str,
    image: u8,
    w: usize,
    h: usize,
    level: u8,
    fec: bool,
    mcus: &[Mcu420],
) -> Vec<[u8; PACKET]> {
    let pl = if fec { PAYLOAD_FEC } else { PAYLOAD_NOFEC };
    let mut coded = Vec::new();
    let mut stream: Vec<u8> = Vec::new();
    let mut first: BTreeMap<usize, (usize, usize)> = BTreeMap::new(); // packet → (offset, mcu index)
    let mut dc = [0i32; 3];
    for (k, m) in mcus.iter().enumerate() {
        let start = stream.len();
        let pk = start / pl;
        if let std::collections::btree_map::Entry::Vacant(e) = first.entry(pk) {
            e.insert((start % pl, k));
            dc = [0; 3];
        }
        coded.clear();
        let mut bw = EntropyWriter::new(&mut coded, false);
        encode_mcu(&mut bw, m, &mut dc);
        bw.align();
        stream.extend_from_slice(&coded);
    }
    let n = stream.len().div_ceil(pl).max(1);
    stream.resize(n * pl, 0xFF);
    let rs = Rs::new(CCSDS_255_223_CONVENTIONAL).expect("SSDV code");
    (0..n)
        .map(|k| {
            let mut p = [0u8; PACKET];
            p[0] = 0x55;
            p[1] = if fec { 0x66 } else { 0x67 };
            p[2..6].copy_from_slice(&encode_call(call).to_be_bytes());
            p[6] = image;
            p[7..9].copy_from_slice(&(k as u16).to_be_bytes());
            p[9] = (w / 16) as u8;
            p[10] = (h / 16) as u8;
            p[11] = (level & 7) | if k + 1 == n { 8 } else { 0 };
            let (off, idx) = first.get(&k).copied().unwrap_or((0xFF, 0xFFFF));
            p[12] = off as u8;
            p[13..15].copy_from_slice(&(idx as u16).to_be_bytes());
            p[HDR..HDR + pl].copy_from_slice(&stream[k * pl..(k + 1) * pl]);
            let c = CRC32_ISO_HDLC.checksum(&p[1..HDR + pl]);
            p[HDR + pl..HDR + pl + 4].copy_from_slice(&c.to_be_bytes());
            if fec {
                let (data, parity) = p[1..].split_at_mut(223);
                let _ = rs.encode(data, parity);
            }
            p
        })
        .collect()
}

/// One received, checked packet.
#[derive(Clone, Debug, PartialEq)]
pub struct SsdvPacket {
    /// The bytes.
    pub bytes: [u8; PACKET],
    /// Signal time.
    pub time: f64,
    /// RS corrections.
    pub corrected: usize,
    /// Bit rate.
    pub baud: u32,
    /// 8N2 framing.
    pub n82: bool,
    /// Inverted polarity.
    pub inverted: bool,
}

impl Item for SsdvPacket {
    const KIND: &'static str = "ssdv.packet";
}

/// One picture being assembled.
#[derive(Clone, Debug)]
pub struct Assembly {
    /// Call sign.
    pub call: String,
    /// Image id.
    pub image: u8,
    /// Width, height.
    pub width: u16,
    /// Height.
    pub height: u16,
    /// Quality level.
    pub quality: u8,
    /// Packets held by id.
    pub packets: BTreeMap<u16, [u8; PACKET]>,
    /// The last packet's id when the end flag was seen.
    pub last: Option<u16>,
    /// Signal time of the first packet, s.
    pub first_t: f64,
    /// Last arrival.
    pub last_t: f64,
    /// Grows with every packet added.
    pub revision: u32,
    /// The JPEG now.
    pub jpeg: Arc<[u8]>,
}

impl Assembly {
    /// Packet numbers missing before the last (or the highest held).
    pub fn missing(&self) -> Vec<u16> {
        let top = self
            .last
            .or_else(|| self.packets.keys().next_back().copied())
            .unwrap_or(0);
        (0..=top)
            .filter(|k| !self.packets.contains_key(k))
            .collect()
    }
    /// All packets 0 … last are held.
    pub fn complete(&self) -> bool {
        self.last.is_some() && self.missing().is_empty()
    }
    /// Adds a packet (a duplicate is ignored); returns whether it was new.
    pub fn add(&mut self, p: &[u8; PACKET], t: f64) -> bool {
        let h = header(p);
        if self.packets.contains_key(&h.packet) {
            return false;
        }
        self.packets.insert(h.packet, *p);
        if h.eoi {
            self.last = Some(h.packet);
        }
        self.last_t = t;
        self.revision += 1;
        self.jpeg = rebuild(self).into();
        true
    }
    /// A new assembly from a first packet.
    pub fn new(p: &[u8; PACKET], t: f64) -> Assembly {
        let h = header(p);
        let mut a = Assembly {
            call: h.call,
            image: h.image,
            width: h.width,
            height: h.height,
            quality: h.quality,
            packets: BTreeMap::new(),
            last: None,
            first_t: t,
            last_t: t,
            revision: 0,
            jpeg: Arc::from(Vec::new()),
        };
        a.add(p, t);
        a
    }
}

/// Rebuilds the JPEG from the packets held: each run of consecutive packets is decoded from the first MCU that starts in it.
/// Missing MCUs are grey. An empty picture (no whole MCU) returns an empty file.
pub fn rebuild(a: &Assembly) -> Vec<u8> {
    let (w, h) = (a.width as usize, a.height as usize);
    let total = (w / 16) * (h / 16);
    let mut mcus: Vec<Option<Mcu420>> = vec![None; total];
    let mut scratch = Vec::new();
    let ids: Vec<u16> = a.packets.keys().copied().collect();
    let mut i = 0;
    while i < ids.len() {
        // A run of consecutive packets.
        let mut j = i;
        while j + 1 < ids.len() && ids[j + 1] == ids[j] + 1 {
            j += 1;
        }
        let run: Vec<&[u8; PACKET]> = ids[i..=j].iter().map(|k| &a.packets[k]).collect();
        let pl = payload(run[0]).len();
        let mut data = Vec::with_capacity(pl * run.len());
        for p in &run {
            data.extend_from_slice(payload(p));
        }
        // MCU starts that reset the predictors: (byte position in the run, MCU index).
        let starts: Vec<(usize, usize)> = run
            .iter()
            .enumerate()
            .filter_map(|(k, p)| {
                let hd = header(p);
                (hd.mcu_offset != 0xFF)
                    .then_some((k * pl + hd.mcu_offset as usize, hd.mcu_index as usize))
            })
            .collect();
        if let Some(&(pos0, idx0)) = starts.first() {
            let mut pos = pos0;
            let mut idx = idx0;
            let mut dc = [0i32; 3];
            while idx < total && pos < data.len() {
                if let Some(&(_, si)) = starts.iter().find(|s| s.0 == pos) {
                    dc = [0; 3];
                    idx = si;
                }
                match decode_mcu(&data[pos..], &mut dc, &mut scratch) {
                    Some((m, len)) => {
                        mcus[idx] = Some(m);
                        idx += 1;
                        pos += len;
                    }
                    None => break,
                }
            }
        }
        i = j + 1;
    }
    write_mcus_420(a.width, a.height, quality_of_level(a.quality), &mcus).unwrap_or_default()
}

/// The FM front end (`ssdv.fm`: iq to the discriminator output in Hz at about 96 kHz). It keeps its own short filter: the
/// graph's channel component starts with a longer transient that holds the bit slicers' thresholds, and it loses 8N2 packets.
pub struct FmFront {
    osc: Nco,
    dec: Option<FirDecimator<Complex32>>,
    disc: Discriminator,
}

impl Default for FmFront {
    fn default() -> Self {
        FmFront {
            osc: Nco::new(1.0, 0.0),
            dec: None,
            disc: Discriminator::new(1.0, 1.0).expect("positive values"),
        }
    }
}

impl Component for FmFront {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor("ssdv.fm", vec![Port::iq("in")], vec![Port::real("out")])
    }
    fn values(&self) -> Values {
        sdrkit_graph::no_values()
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let f = &inputs[0];
        let d = ((f.rate / 96e3).floor() as usize).max(1);
        let fo = f.rate / d as f64;
        self.osc = Nco::new(f.rate, -f.signal_offset_hz());
        self.dec = Some(FirDecimator::new(
            &lowpass(8 * d + 9, 0.45 / d as f64 * 0.5),
            d,
        ));
        self.disc = Discriminator::new(fo, 1.0)?;
        Ok(vec![Format::new(Kind::Real, fo)])
    }
    fn process(&mut self, io: &mut Io) {
        for t in io.input(0).tags() {
            if let sdrkit_graph::TagKind::Retuned {
                centre_hz,
                tuned_hz,
            } = t.kind
            {
                self.osc.set_freq(-(tuned_hz - centre_hz));
            }
        }
        let x = io.iq(0);
        let Some(dec) = self.dec.as_mut() else { return };
        let out = io.real_out(0);
        for &s in x {
            if let Some(y) = dec.push(s * self.osc.next_sample()) {
                out.push(self.disc.process_sample(y));
            }
        }
    }
}

/// A bit slicer: a boxcar over one bit, a mid-level threshold from fast-attack slow-decay peak trackers, the phase nudged at
/// every level change.
#[derive(Clone, Debug)]
pub struct BitSlicer {
    spb: f64,
    boxbuf: Vec<f32>,
    pos: usize,
    sum: f32,
    hi: f32,
    lo: f32,
    phase: f64,
    last: bool,
    since: u64,
    pending: Option<f64>,
    pend_n: u32,
}

impl BitSlicer {
    /// For `baud` at `rate`.
    pub fn new(rate: f64, baud: f64) -> BitSlicer {
        let spb = rate / baud;
        BitSlicer {
            spb,
            boxbuf: vec![0.0; (spb.round() as usize).max(1)],
            pos: 0,
            sum: 0.0,
            hi: 0.0,
            lo: 0.0,
            phase: 0.0,
            last: false,
            since: 0,
            pending: None,
            pend_n: 0,
        }
    }
    /// One sample; a bit when one is decided.
    #[inline]
    pub fn push(&mut self, x: f32) -> Option<bool> {
        let n = self.boxbuf.len();
        self.sum += x - self.boxbuf[self.pos];
        self.boxbuf[self.pos] = x;
        self.pos += 1;
        if self.pos == n {
            self.pos = 0;
            self.sum = self.boxbuf.iter().sum();
        }
        let y = self.sum / n as f32;
        // The trackers move toward each other only while the level keeps changing, so a long run of equal bits (padding) does
        // not drag the threshold, and a start-up transient is forgotten within a preamble.
        self.since += 1;
        let decay = if (self.since as f64) < 16.0 * self.spb {
            1.0 / (20.0 * self.spb as f32)
        } else {
            0.0
        };
        if y > self.hi {
            self.hi = y;
        } else {
            self.hi -= (self.hi - self.lo) * decay;
        }
        if y < self.lo {
            self.lo = y;
        } else {
            self.lo += (self.hi - self.lo) * decay;
        }
        let b = y > 0.5 * (self.hi + self.lo);
        // A level change counts once it has lasted a third of a bit (an FM click is shorter); the phase is nudged from where it
        // began.
        if b != self.last {
            let started = *self.pending.get_or_insert(self.phase);
            self.pend_n += 1;
            if self.pend_n as f64 >= self.spb / 3.0 {
                self.last = b;
                self.since = 0;
                self.pending = None;
                self.pend_n = 0;
                let err = (started - 0.5 + 0.5).rem_euclid(1.0) - 0.5;
                self.phase -= 0.3 * err;
            }
        } else {
            self.pending = None;
            self.pend_n = 0;
        }
        self.phase += 1.0 / self.spb;
        if self.phase >= 1.0 {
            self.phase -= 1.0;
            Some(b)
        } else {
            None
        }
    }
}

struct Capture {
    bytes: Vec<u8>,
    counted: bool,
}

/// A plain-stream packet reader: 16-bit sync 0x5566 / 0x5567 with at most one bit error, MSB first.
struct PlainReader {
    baud: u32,
    inverted: bool,
    shift: u32,
    nbits: u32,
    byte_acc: u8,
    caps: Vec<(Capture, u32, u8)>,
    bad: u64,
}

/// An 8N2 reader: UART bytes (start 0, 8 data LSB first, stop 1), the pair 0x55 0x66 / 0x55 0x67.
struct UartReader {
    baud: u32,
    inverted: bool,
    state: i32,
    acc: u16,
    prev: Option<u8>,
    caps: Vec<Capture>,
}

/// The slicer bank (`ssdv.slicers`: real to `ssdv.packet`).
pub struct Slicers {
    rate: f64,
    choice: Speed,
    plain: Vec<(BitSlicer, Vec<PlainReader>)>,
    uart: Vec<(BitSlicer, Vec<UartReader>)>,
    /// Packets refused.
    pub bad: u64,
    t: f64,
    checker: PacketCheck,
}

impl Default for Slicers {
    fn default() -> Self {
        Slicers {
            rate: 0.0,
            choice: BAUD_SETTING.default,
            plain: Vec::new(),
            uart: Vec::new(),
            bad: 0,
            t: 0.0,
            checker: PacketCheck::new(),
        }
    }
}

fn finish(
    checker: &mut PacketCheck,
    bytes: &[u8],
    t: f64,
    baud: u32,
    n82: bool,
    inverted: bool,
    out: &mut Vec<Timed<SsdvPacket>>,
) -> bool {
    let mut p = [0u8; PACKET];
    p.copy_from_slice(&bytes[..PACKET]);
    match checker.check(&mut p) {
        Ok(n) => {
            out.push(Timed {
                time: t,
                value: SsdvPacket {
                    bytes: p,
                    time: t,
                    corrected: n,
                    baud,
                    n82,
                    inverted,
                },
            });
            true
        }
        Err(_) => false,
    }
}

impl Slicers {
    fn build(&mut self) {
        self.plain.clear();
        self.uart.clear();
        let rate = self.rate;
        let plain = [1200u32, 2400, 4800, 9600, 19200];
        let n82 = [300u32, 600, 1200];
        let choice = self.choice;
        let want = |b: u32, u: bool| {
            choice == Speed::Auto
                || choice.framing()
                    == Some(if u {
                        Framing::N82(b)
                    } else {
                        Framing::Plain(b)
                    })
        };
        for &b in &plain {
            if want(b, false) && rate / b as f64 >= 4.0 {
                let readers = [false, true]
                    .iter()
                    .map(|&inv| PlainReader {
                        baud: b,
                        inverted: inv,
                        shift: 0,
                        nbits: 0,
                        byte_acc: 0,
                        caps: Vec::new(),
                        bad: 0,
                    })
                    .collect();
                self.plain.push((BitSlicer::new(rate, b as f64), readers));
            }
        }
        for &b in &n82 {
            if want(b, true) {
                let readers = [false, true]
                    .iter()
                    .map(|&inv| UartReader {
                        baud: b,
                        inverted: inv,
                        state: -1,
                        acc: 0,
                        prev: None,
                        caps: Vec::new(),
                    })
                    .collect();
                self.uart.push((BitSlicer::new(rate, b as f64), readers));
            }
        }
    }
}

impl Component for Slicers {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "ssdv.slicers",
            vec![Port::real("in")],
            vec![Port::typed::<SsdvPacket>("packets").rate_from(RateFrom::Fixed(0.0))],
        )
        .with_settings(SlicersSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.rate = inputs[0].rate;
        self.build();
        Ok(vec![Format::new(Kind::typed::<SsdvPacket>(), 0.0)])
    }
    fn process(&mut self, io: &mut Io) {
        let x = io.real(0);
        let first = io.input(0).first();
        let out = io.items_out::<SsdvPacket>(0);
        for (i, &v) in x.iter().enumerate() {
            let t = (first + i as u64) as f64 / self.rate;
            for (sl, readers) in &mut self.plain {
                let Some(b) = sl.push(v) else { continue };
                for r in readers.iter_mut() {
                    let bit = b ^ r.inverted;
                    r.shift = (r.shift << 1) | bit as u32;
                    // Captures in progress.
                    let mut done = Vec::new();
                    for (k, (cap, nb, acc)) in r.caps.iter_mut().enumerate() {
                        *acc = (*acc << 1) | bit as u8;
                        *nb += 1;
                        if *nb == 8 {
                            cap.bytes.push(*acc);
                            *nb = 0;
                            *acc = 0;
                            if cap.bytes.len() == PACKET {
                                done.push(k);
                            }
                        }
                    }
                    for &k in done.iter().rev() {
                        let (cap, _, _) = r.caps.remove(k);
                        if !finish(
                            &mut self.checker,
                            &cap.bytes,
                            t,
                            r.baud,
                            false,
                            r.inverted,
                            out,
                        ) && cap.counted
                        {
                            r.bad += 1;
                        }
                    }
                    let w = (r.shift & 0xFFFF) as u16;
                    for ty in [0x5566u16, 0x5567] {
                        let d = (w ^ ty).count_ones();
                        if d <= 1 && r.caps.len() < 6 {
                            // Count as bad only with an exact sync and a 0x55 preamble byte before it.
                            let pre = ((r.shift >> 16) & 0xFF) as u8;
                            let counted = d == 0 && (pre ^ 0x55).count_ones() <= 1;
                            r.caps.push((
                                Capture {
                                    bytes: vec![0x55, (ty & 0xFF) as u8],
                                    counted,
                                },
                                0,
                                0,
                            ));
                        }
                    }
                    let _ = r.nbits;
                    let _ = r.byte_acc;
                }
            }
            for (sl, readers) in &mut self.uart {
                let Some(b) = sl.push(v) else { continue };
                for r in readers.iter_mut() {
                    let bit = b ^ r.inverted;
                    if r.state < 0 {
                        if !bit {
                            r.state = 0;
                            r.acc = 0;
                        }
                        continue;
                    }
                    if r.state < 8 {
                        r.acc |= (bit as u16) << r.state;
                        r.state += 1;
                        continue;
                    }
                    // Stop bit.
                    r.state = -1;
                    if !bit {
                        r.prev = None;
                        continue; // framing error: the byte is dropped
                    }
                    let byte = r.acc as u8;
                    let mut done = Vec::new();
                    for (k, cap) in r.caps.iter_mut().enumerate() {
                        cap.bytes.push(byte);
                        if cap.bytes.len() == PACKET {
                            done.push(k);
                        }
                    }
                    for &k in done.iter().rev() {
                        let cap = r.caps.remove(k);
                        finish(
                            &mut self.checker,
                            &cap.bytes,
                            t,
                            r.baud,
                            true,
                            r.inverted,
                            out,
                        );
                    }
                    if r.prev == Some(0x55) && (byte == 0x66 || byte == 0x67) && r.caps.len() < 6 {
                        r.caps.push(Capture {
                            bytes: vec![0x55, byte],
                            counted: true,
                        });
                    }
                    r.prev = Some(byte);
                }
            }
            self.t = t;
        }
        // The same packet can come from both polarities or two speeds: keep one.
        out.dedup_by(|a, b| a.value.bytes == b.value.bytes);
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match SlicersSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            SlicersSetting::Baud => self.choice = BAUD_SETTING.read(id, v)?,
        }
        if self.rate > 0.0 {
            self.build();
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([(SlicersSetting::Baud, Value::Choice(self.choice.index()))])
    }
}

/// A row of the packets table.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PacketRow {
    /// Signal time, s.
    pub time: f64,
    /// The host time, when the samples are live.
    pub host_time: Option<f64>,
    /// The header.
    pub header: Header,
    /// Bytes corrected by the RS code.
    pub corrected: usize,
    /// The speed.
    pub baud: u32,
    /// 8N2 bytes.
    pub n82: bool,
    /// Inverted.
    pub inverted: bool,
}

impl Shift for PacketRow {
    fn shift_times(&mut self, by_s: f64) {
        self.time += by_s;
    }
}

/// A picture as saved state: its packets.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SavedPicture {
    /// The packets, in order of their numbers.
    pub packets: Vec<Vec<u8>>,
    /// Signal time of the first packet, s.
    pub first_t: f64,
    /// Signal time of the last packet, s.
    pub last_t: f64,
}

impl Shift for SavedPicture {
    fn shift_times(&mut self, by_s: f64) {
        self.first_t += by_s;
        self.last_t += by_s;
    }
}

/// Packets listed.
pub const MAX_PACKETS: usize = 400;
/// Pictures held.
pub const MAX_PICTURES: usize = 8;

/// What the picture assembler keeps: the packet log and the packets of each picture.
#[derive(Default, Serialize, Deserialize)]
pub struct Store {
    /// The packets, oldest first.
    pub packets: Vec<PacketRow>,
    /// The pictures, oldest first.
    pub pictures: Vec<SavedPicture>,
}

impl Shift for Store {
    fn shift_times(&mut self, by_s: f64) {
        self.packets.iter_mut().for_each(|p| p.shift_times(by_s));
        self.pictures.iter_mut().for_each(|p| p.shift_times(by_s));
    }
}

impl Tables for Store {
    const FORMAT: StateFormat = StateFormat::new("science.ssdv_pictures", 1);
    fn clear(&mut self) {
        self.packets.clear();
        self.pictures.clear();
    }
    fn adopt(&mut self, saved: Self) {
        *self = saved;
    }
}

/// What `ssdv` publishes.
pub fn declaration() -> ResultsDecl {
    crate::typed::shape("ssdv").results_decl()
}

impl Shaped for PacketRow {
    type Row = SsdvPacketRow<'static>;
    fn row(&self, _seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        let h = &self.header;
        out.put(&SsdvPacketRow {
            time_s: self.time,
            host_time: self.host_time.map(Time),
            call: &h.call,
            image: i64::from(h.image),
            packet: i64::from(h.packet),
            rs: self.corrected as i64,
            baud: i64::from(self.baud),
            n82: self.n82,
            inverted: self.inverted,
            no_fec: !h.fec,
            eoi: h.eoi,
        });
    }
}

sdrkit_core::commands! {
    /// The commands of [`Pictures`].
    pub enum PicturesCommand {
        /// Forgets every packet and picture.
        Clear = objcmd::CLEAR,
    }
}

/// The picture assembler (`ssdv.pictures`: `ssdv.packet` to `picture`).
pub struct Pictures {
    pics: Vec<Assembly>,
    /// The numbers of the pictures that still grow, by call sign and image id.
    ids: PictureIds<(String, u8)>,
    rows: LogRing<PacketRow>,
    good: u64,
    bad: u64,
    corrected: u64,
    done: u64,
    last: Option<(u32, bool)>,
    last_t: f64,
    t: f64,
    rate: f64,
    live: bool,
    clock: Clock,
    published: Published,
    shown: Status,
    key: String,
}

impl Default for Pictures {
    fn default() -> Self {
        Pictures {
            pics: Vec::new(),
            ids: PictureIds::new(),
            rows: LogRing::new(MAX_PACKETS),
            good: 0,
            bad: 0,
            corrected: 0,
            done: 0,
            last: None,
            last_t: f64::NEG_INFINITY,
            t: 0.0,
            rate: 0.0,
            live: false,
            clock: Clock::fixed(0.0),
            published: Published::new(SsdvFields::SHAPE.fields).with_tables(&[SSDV_PACKETS.def(), SSDV_PICTURES.def()]),
            shown: Status::default(),
            key: String::new(),
        }
    }
}

impl Pictures {
    /// The pictures held, oldest first.
    pub fn pictures(&self) -> &[Assembly] {
        &self.pics
    }

    fn publish(&mut self) {
        let state = if self.t - self.last_t < 30.0 { 2 } else { 0 };
        let (good, done, last) = (self.good, self.done, self.last);
        status(
            &mut self.shown,
            lock_state_of(state),
            None,
            None,
            None,
            |s| {
                let _ = match last {
                    Some((b, n82)) if good > 0 => write!(
                        s,
                        "SSDV: {good} packets, {done} pictures done, {b}{} baud",
                        if n82 { " 8N2" } else { "" }
                    ),
                    _ => write!(s, "SSDV: no signal"),
                };
            },
        );
        let p = &mut self.published;
        p.results_mut().set_status(&self.shown);
        p.put(&SsdvFields {
            packets_ok: self.good as i64,
            packets_bad: self.bad as i64,
            corrected: self.corrected as i64,
            pictures_done: self.done as i64,
            last_baud: self.last.map(|(b, _)| i64::from(b)),
            last_n82: self.last.map(|(_, n82)| n82),
        });
        log_rows(&self.rows, &SSDV_PACKETS, self.t, p);
        for a in &self.pics {
            picture_key(&mut self.key, a);
            SSDV_PICTURES.put(
                p,
                &self.key,
                &SsdvPictureRow {
                    call: &a.call,
                    image: i64::from(a.image),
                    width: i64::from(a.width),
                    height: i64::from(a.height),
                    quality: i64::from(a.quality),
                    held: a.packets.len() as i64,
                    missing: a.missing().len() as i64,
                    complete: a.complete(),
                    bytes: a.jpeg.len() as i64,
                },
            );
        }
    }

    fn store(&self) -> Store {
        Store {
            packets: self.rows.iter().cloned().collect(),
            pictures: self
                .pics
                .iter()
                .map(|a| SavedPicture {
                    packets: a.packets.values().map(|p| p.to_vec()).collect(),
                    first_t: a.first_t,
                    last_t: a.last_t,
                })
                .collect(),
        }
    }

    fn adopt(&mut self, s: Store) {
        self.rows.clear();
        for r in s.packets {
            self.rows.push(r);
        }
        self.pics.clear();
        for sp in s.pictures {
            let mut a: Option<Assembly> = None;
            for p in &sp.packets {
                let Ok(p) = <[u8; PACKET]>::try_from(p.as_slice()) else {
                    continue;
                };
                match a.as_mut() {
                    Some(a) => {
                        a.add(&p, sp.last_t);
                    }
                    None => a = Some(Assembly::new(&p, sp.first_t)),
                }
            }
            if let Some(a) = a {
                self.pics.push(a);
            }
        }
    }
}

/// The row key of a picture: its call sign and image id.
fn picture_key(key: &mut String, a: &Assembly) {
    key.clear();
    let _ = write!(key, "{}-{}", a.call, a.image);
}

impl Component for Pictures {
    fn any_ref(&self) -> Option<&dyn std::any::Any> {
        Some(self)
    }
    fn any_mut(&mut self) -> Option<&mut dyn std::any::Any> {
        Some(self)
    }
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "ssdv.pictures",
            vec![Port::iq("in"), Port::typed::<SsdvPacket>("packets")],
            vec![Port::typed::<Picture>("pictures").rate_from(RateFrom::Fixed(0.0))],
        )
    }
    fn attach(&mut self, ctx: &RunContext) {
        self.live = ctx.wall_clock();
        self.clock = ctx.clock.clone();
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.rate = inputs[0].rate;
        self.publish();
        Ok(vec![Format::new(Kind::typed::<Picture>(), 0.0)])
    }
    fn finish(&mut self, io: &mut Io) {
        self.process(io);
        // The end of the input ends the pictures that still wait for packets.
        for ((call, image), id) in self.ids.close_all() {
            let Some(a) = self
                .pics
                .iter()
                .find(|a| a.call == call && a.image == image)
            else {
                continue;
            };
            io.items_out::<Picture>(0).push(Timed {
                time: self.t,
                value: Picture {
                    codec: Codec::Jpeg,
                    layout: None,
                    name: Some(format!("{call}-{image}")),
                    id,
                    state: PictureState::Finished,
                    data: a.jpeg.clone(),
                },
            });
        }
    }
    fn process(&mut self, io: &mut Io) {
        let first = io.input(0).first();
        self.t = (first + io.input_len(0) as u64) as f64 / self.rate.max(1.0);
        let packets = io.items::<SsdvPacket>(1);
        let outp = io.items_out::<Picture>(0);
        for p in packets {
            let pk = &p.value;
            let h = header(&pk.bytes);
            self.good += 1;
            self.corrected += (pk.corrected > 0) as u64;
            self.last = Some((pk.baud, pk.n82));
            self.last_t = pk.time;
            self.rows.push(PacketRow {
                time: pk.time,
                host_time: wall(self.live, &self.clock),
                header: h.clone(),
                corrected: pk.corrected,
                baud: pk.baud,
                n82: pk.n82,
                inverted: pk.inverted,
            });
            let i = match self
                .pics
                .iter()
                .position(|a| a.call == h.call && a.image == h.image)
            {
                Some(i) => {
                    let was = self.pics[i].complete();
                    if !self.pics[i].add(&pk.bytes, pk.time) {
                        continue;
                    }
                    if !was && self.pics[i].complete() {
                        self.done += 1;
                        self.published.results_mut().log(format!(
                            "SSDV: picture {} from {} is complete",
                            h.image, h.call
                        ));
                    }
                    i
                }
                None => {
                    if self.pics.len() >= MAX_PICTURES {
                        let gone = self.pics.remove(0);
                        // A picture that leaves before its last packet goes out once more, finished.
                        if let Some(id) = self.ids.close(&(gone.call.clone(), gone.image)) {
                            outp.push(Timed {
                                time: pk.time,
                                value: Picture {
                                    codec: Codec::Jpeg,
                                    layout: None,
                                    name: Some(format!("{}-{}", gone.call, gone.image)),
                                    id,
                                    state: PictureState::Finished,
                                    data: gone.jpeg.clone(),
                                },
                            });
                        }
                        picture_key(&mut self.key, &gone);
                        self.published.remove_row(&SSDV_PICTURES, &self.key);
                    }
                    self.pics.push(Assembly::new(&pk.bytes, pk.time));
                    let i = self.pics.len() - 1;
                    if self.pics[i].complete() {
                        self.done += 1;
                    }
                    i
                }
            };
            let a = &self.pics[i];
            let name = match wall(self.live, &self.clock) {
                Some(w) => format!("{}-{}-{}", a.call, a.image, calendar::compact(w as i64)),
                None => format!("{}-{}", a.call, a.image),
            };
            // One number for each picture of a station. The picture grows until its last packet.
            let key = (a.call.clone(), a.image);
            let id = self.ids.of(&key);
            let state = if a.complete() {
                self.ids.close(&key);
                PictureState::Finished
            } else {
                PictureState::Growing
            };
            outp.push(Timed {
                time: pk.time,
                value: Picture {
                    codec: Codec::Jpeg,
                    layout: None,
                    name: Some(name),
                    id,
                    state,
                    data: a.jpeg.clone(),
                },
            });
        }
        if !outp.is_empty() || io.tick() {
            self.publish();
        }
    }
    fn values(&self) -> Values {
        sdrkit_graph::no_values()
    }
    fn commands(&self) -> &Commands {
        PicturesCommand::commands()
    }
    fn invoke(&mut self, id: CommandId, _args: &Values) -> Result<Option<Record>, CommandError> {
        match PicturesCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            PicturesCommand::Clear => {
                self.pics.clear();
                self.rows.clear();
                self.published.clear_rows(&SSDV_PICTURES);
            }
        }
        self.publish();
        Ok(None)
    }
    fn results(&self) -> Option<&Results> {
        Some(self.published.results())
    }
    fn save_state(&self) -> Option<StateBlob> {
        self.store().save_state(self.t).ok()
    }
    fn restore_state(&mut self, state: &StateBlob) -> Result<(), StateError> {
        let mut s = Store::default();
        s.restore_state(state, self.t)?;
        self.adopt(s);
        self.published.clear_rows(&SSDV_PICTURES);
        self.publish();
        Ok(())
    }
}

sdrkit_core::choice_enum! {
    /// The speed and framing the slicers read.
    pub enum Speed {
        /// Every speed and framing at once.
        Auto = ("auto", "Automatic (all at once)"),
        /// 300 baud 8N2 bytes.
        B300N82 = ("300_8n2", "300 (8N2)"),
        /// 600 baud 8N2 bytes.
        B600N82 = ("600_8n2", "600 (8N2)"),
        /// 1200 baud 8N2 bytes.
        B1200N82 = ("1200_8n2", "1200 (8N2)"),
        /// 1200 baud.
        B1200 = ("1200", "1200"),
        /// 2400 baud.
        B2400 = ("2400", "2400"),
        /// 4800 baud.
        B4800 = ("4800", "4800"),
        /// 9600 baud.
        B9600 = ("9600", "9600"),
        /// 19200 baud.
        B19200 = ("19200", "19200"),
        /// 115200 baud (Wenet).
        B115200 = ("115200", "115200 (Wenet)"),
    }
}

impl Speed {
    /// The framing of a fixed speed, `None` for the automatic choice.
    pub fn framing(self) -> Option<Framing> {
        Some(match self {
            Speed::Auto => return None,
            Speed::B300N82 => Framing::N82(300),
            Speed::B600N82 => Framing::N82(600),
            Speed::B1200N82 => Framing::N82(1200),
            Speed::B1200 => Framing::Plain(1200),
            Speed::B2400 => Framing::Plain(2400),
            Speed::B4800 => Framing::Plain(4800),
            Speed::B9600 => Framing::Plain(9600),
            Speed::B19200 => Framing::Plain(19200),
            Speed::B115200 => Framing::Plain(115_200),
        })
    }
}

/// The speed the slicers read.
pub const BAUD_SETTING: Setting<field::Choice<Speed>> =
    Setting::choice("baud", "Speed", Speed::Auto);
/// The speed of the test signal.
pub const GEN_MODE: Setting<field::Choice<Speed>> =
    Setting::choice("gen_mode", "Test signal: speed", Speed::B9600)
        .section("Test signal")
        .advanced();

sdrkit_core::settings! {
    /// The settings of [`Slicers`].
    pub enum SlicersSetting {
        /// The speed the slicers read.
        Baud = BAUD_SETTING,
    }
}

sdrkit_core::settings! {
    /// The settings of the SSDV stack.
    pub enum SsdvSetting {
        /// The speed the slicers read.
        Baud = BAUD_SETTING,
        /// The speed of the test signal.
        GenMode = GEN_MODE,
    }
}

/// The lowest input rate.
pub const MIN_RATE: f64 = 250e3;

/// The receiver: the FM front end, the slicers and the picture assembler.
///
/// # Errors
///
/// The errors of [`GraphBuilder::build`] and [`StackGraphBuilder::build`](sdrkit_graph::StackGraphBuilder::build).
pub fn receiver(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    let graph = GraphBuilder::new()
        .add("fm", FmFront::default())
        .add("slicers", Slicers::default())
        .add("pictures", Pictures::default())
        .input("fm")
        .input("pictures.in")
        .connect("fm", "slicers")
        .connect("slicers", "pictures.packets")
        .output("pictures")
        .clock(REPORT_S)
        .status_from("pictures")
        .declare(&declaration())
        .catalog(&Catalog::of(cfg)?)
        .build()?;
    StackGraph::of(crate::typed::shape("ssdv"), graph)
        .min_rate(MIN_RATE, "SSDV needs a sample rate of at least 250 ksps")
        .route(BAUD_SETTING.key(), &["slicers.baud"])?
        .command(
            objcmd::CLEAR.key,
            sdrkit_graph::CommandTarget::of("pictures", "clear"),
        )
        .with_settings(&cfg.settings)
        .build()
}

/// The receiver as the engine takes it.
pub fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    crate::stack_receiver(receiver(cfg))
}

/// The test picture: a gradient with a few shapes.
pub fn test_picture(w: usize, h: usize) -> Vec<u8> {
    let mut rgb = vec![0u8; w * h * 3];
    for y in 0..h {
        for x in 0..w {
            let i = 3 * (y * w + x);
            let (cx, cy) = (x as i32 - w as i32 / 3, y as i32 - h as i32 / 2);
            let disc = cx * cx + cy * cy < (h as i32 / 4).pow(2);
            let square = x > 2 * w / 3 && x < 2 * w / 3 + w / 6 && y > h / 4 && y < h / 2;
            rgb[i] = if disc { 230 } else { (255 * x / w) as u8 };
            rgb[i + 1] = if square { 40 } else { (255 * y / h) as u8 };
            rgb[i + 2] = if disc { 40 } else { 128 };
        }
    }
    rgb
}

/// The packets of the test picture.
pub fn test_packets(
    call: &str,
    image: u8,
    w: usize,
    h: usize,
    level: u8,
    fec: bool,
) -> Vec<[u8; PACKET]> {
    let rgb = test_picture(w, h);
    let m = rgb_to_mcus_420(&rgb, w, h, quality_of_level(level));
    make_packets(call, image, w, h, level, fec, &m)
}

/// How the generator frames the bits.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Framing {
    /// A plain bit stream (MSB first) at this rate.
    Plain(u32),
    /// 8N2 bytes at this rate.
    N82(u32),
}

/// The SSDV generator: packets of the test picture as continuous-phase 2-FSK at 0 Hz with amplitude 1.
pub struct SsdvGenerator {
    rate: f64,
    bits: Vec<bool>,
    baud: f64,
    cpm: Cpm,
    k: usize,
    buf: Vec<Complex32>,
    pos: usize,
    levels: Vec<f32>,
    inverted: bool,
    looped: bool,
    ended: bool,
}

impl SsdvGenerator {
    /// The packets framed and keyed. The deviation is half the bit rate (at least 1200 Hz) for plain streams and 1.5 times the
    /// bit rate (at least 500 Hz) for 8N2 bytes.
    pub fn new(
        rate: f64,
        packets: &[[u8; PACKET]],
        framing: Framing,
        inverted: bool,
    ) -> SsdvGenerator {
        let mut bytes = vec![0x55u8; 16];
        for p in packets {
            bytes.extend_from_slice(p);
            bytes.extend_from_slice(&[0x55; 4]);
        }
        let mut bits = Vec::new();
        let (baud, dev) = match framing {
            Framing::Plain(b) => {
                for &x in &bytes {
                    for k in (0..8).rev() {
                        bits.push((x >> k) & 1 == 1);
                    }
                }
                (b as f64, (b as f64 / 2.0).max(1200.0))
            }
            Framing::N82(b) => {
                for &x in &bytes {
                    bits.push(false);
                    for k in 0..8 {
                        bits.push((x >> k) & 1 == 1);
                    }
                    bits.push(true);
                    bits.push(true);
                }
                (b as f64, (1.5 * b as f64).max(500.0))
            }
        };
        let cpm = Cpm::cpfsk(2.0 * dev / baud, rate / baud).expect("a valid 2-FSK");
        SsdvGenerator {
            rate,
            bits,
            baud,
            cpm,
            k: 0,
            buf: Vec::new(),
            pos: 0,
            levels: Vec::with_capacity(256),
            inverted,
            looped: false,
            ended: false,
        }
    }

    /// The same, sent again and again.
    pub fn looped(mut self) -> SsdvGenerator {
        self.looped = true;
        self
    }

    /// From the stack's settings. The automatic choice sends 9600 baud.
    pub fn from_config(cfg: &GenConfig) -> SsdvGenerator {
        let framing = cfg
            .settings
            .get(&GEN_MODE)
            .framing()
            .unwrap_or(Framing::Plain(9600));
        let p = test_packets("TEST", 1, 96, 64, 6, true);
        SsdvGenerator::new(cfg.rate_hz, &p, framing, false).looped()
    }

    /// Seconds of signal in one pass.
    pub fn duration(&self) -> f64 {
        self.bits.len() as f64 / self.baud
    }

    fn more(&mut self) {
        self.buf.drain(..self.pos);
        self.pos = 0;
        self.levels.clear();
        while self.levels.len() < 256 {
            if self.k >= self.bits.len() {
                if !self.looped {
                    break;
                }
                self.k = 0;
            }
            let b = self.bits[self.k] ^ self.inverted;
            self.levels.push(if b { 1.0 } else { -1.0 });
            self.k += 1;
        }
        if self.levels.is_empty() {
            self.cpm.flush(&mut self.buf);
            self.ended = true;
        } else {
            self.cpm.push(&self.levels, &mut self.buf);
        }
    }
}

impl Generator for SsdvGenerator {
    fn rate_hz(&self) -> Hz {
        self.rate
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        let mut i = 0;
        while i < out.len() {
            if self.pos == self.buf.len() {
                if self.ended {
                    break;
                }
                self.more();
                continue;
            }
            let take = (self.buf.len() - self.pos).min(out.len() - i);
            out[i..i + take].copy_from_slice(&self.buf[self.pos..self.pos + take]);
            self.pos += take;
            i += take;
        }
        i
    }
    fn signal_info(&self) -> Option<SignalInfo> {
        Some(SignalInfo {
            symbol_rate_hz: Some(self.baud),
            bit_rate_hz: Some(self.baud),
            ..SignalInfo::new(2.0 * self.baud)
        })
    }
    fn station_offset_hz(&self, _tune_offset_hz: Hz) -> Hz {
        0.0
    }
}

/// The generator.
pub fn make_generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(Box::new(SsdvGenerator::from_config(cfg)))
}

/// The verify case: the test picture at 9600 baud.
pub static VERIFY: VerifyHooks = VerifyHooks {
    cases: verify_cases,
    vector_settings: None,
    check_vector: None,
};

fn verify_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let mut c = VerifyCase::new("test signal, 9600 baud", o, 0.0);
    c.seconds = o.seconds.max(6.0);
    c.expect_audio = Some(false);
    c.check = Some(|r| {
        let n = crate::family::field(r, SsdvFields::PICTURES_DONE.key);
        vec![CheckLine::new(
            SsdvFields::PICTURES_DONE.key,
            n >= 1.0,
            format!("{n}, at least 1 expected"),
        )]
    });
    vec![c]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ideal_bits_through_the_slicer() {
        let pk = test_packets("TEST", 3, 96, 64, 6, true);
        let mut bytes = vec![0x55u8; 16];
        for p in &pk {
            bytes.extend_from_slice(p);
            bytes.extend_from_slice(&[0x55; 4]);
        }
        let rate = 96_000.0;
        let spb = rate / 9600.0;
        let mut s = BitSlicer::new(rate, 9600.0);
        let mut bits_in = Vec::new();
        for &x in &bytes {
            for k in (0..8).rev() {
                bits_in.push((x >> k) & 1 == 1);
            }
        }
        let mut out = Vec::new();
        let n = (bits_in.len() as f64 * spb) as usize + 200;
        for i in 0..n {
            let k = (i as f64 / spb) as usize;
            let v = if k < bits_in.len() && bits_in[k] {
                1.0
            } else {
                -1.0
            };
            if let Some(b) = s.push(v) {
                out.push(b);
            }
        }
        // Find the offset and count errors.
        let best = (0..20)
            .map(|o| {
                out.iter()
                    .skip(o)
                    .zip(&bits_in)
                    .filter(|(a, b)| a != b)
                    .count()
            })
            .min()
            .unwrap();
        assert_eq!(best, 0);
    }
}
