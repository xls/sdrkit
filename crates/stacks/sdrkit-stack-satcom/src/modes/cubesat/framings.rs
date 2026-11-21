//! The frame layers of the burst framings of the table receiver: Geoscan, Lucky-7, Spino, USP, NGHam, U482C, Mobitex and
//! CCSDS concatenated. Each has the encoder of the test signal and a [`Framing`] that decodes it from the soft bits behind its
//! sync word. Soft bits are `f32`, positive for a 0 bit.
//!
//! Standard: CCSDS 131.0-B (code, randomiser, Reed-Solomon); USP public description; NGHam protocol description; Mobitex
//! Interface Specification. The Geoscan, Lucky-7 and Spino layouts and the U482C field layout are not confirmed by a public
//! document.

use sdrkit_algo_fec::conv::{codes, ConvCode, Encoder, Ends, Viterbi};
use sdrkit_algo_fec::mobitex;
use sdrkit_algo_fec::rs::{codes as rs_codes, Basis, Rs, RsCode};
use sdrkit_proto_ccsds::channel::{BasisPolicy, ChannelCoder, ChannelConfig, ChannelDecoder};
use sdrkit_proto_ccsds::{aos, marker, randomiser, tm};
use sdrkit_proto_link::csp::Ax100;
use sdrkit_proto_link::hunt::Framing;
use sdrkit_util_bits::crc::catalog::{CRC16_CMS, CRC16_IBM_SDLC, CRC16_XMODEM};
use sdrkit_util_bits::{seq, BitOrder};

/// The Geoscan sync word.
pub const GEOSCAN_SYNC: u32 = 0x930B_51DE;
/// The Lucky-7 sync word.
pub const LUCKY7_SYNC: u16 = 0x2DD4;
/// Lucky-7 frame bytes after the sync word.
pub const LUCKY7_BYTES: usize = 37;
/// The Spino sync word.
pub const SPINO_SYNC: u32 = 0x2EFC_9827;
/// The USP sync word.
pub const USP_SYNC: u64 = 0x5072_F64B_2D90_B1F5;
/// The USP PLS code of a 48-byte block.
pub const USP_PLS_48: u64 = 0x719D_83C9_5342_2DFA;
/// The USP PLS code of a 223-byte block.
pub const USP_PLS_223: u64 = 0x24C8_D69C_0617_78AF;
/// The NGHam sync word.
pub const NGHAM_SYNC: u32 = 0x5DE6_2A7E;
/// The U482C sync word.
pub const U482C_SYNC: u32 = 0xC3AA_6655;
/// The Mobitex sync word.
pub const MOBITEX_SYNC: u16 = 0x5765;
/// The Mobitex-NX sync word.
pub const MOBITEX_NX_SYNC: u16 = 0x0EF0;

/// What a burst framing decoded.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Decoded {
    /// The frame bytes as the frame table lists them.
    pub data: Vec<u8>,
    /// Reed-Solomon bytes corrected; `None` when no Reed-Solomon decided.
    pub rs_corrected: Option<u32>,
    /// What passed.
    pub validated: &'static str,
    /// The Lucky-7 kind, the Mobitex variant or the NGHam flags in words.
    pub kind: Option<&'static str>,
    /// NGHam flags.
    pub flags: Option<u8>,
    /// The Mobitex callsign.
    pub callsign: Option<String>,
}

/// Hard bits of soft bits, packed most significant bit first.
pub fn soft_bytes(bits: &[f32], out: &mut Vec<u8>) {
    out.clear();
    out.extend(
        bits.as_chunks::<8>()
            .0
            .iter()
            .map(|c| c.iter().fold(0u8, |a, &s| (a << 1) | u8::from(s < 0.0))),
    );
}

/// Soft bits as LLRs for the Viterbi decoder, scaled so their mean magnitude is 32.
pub fn llrs(bits: &[f32], out: &mut Vec<i8>) {
    let mean = bits.iter().map(|s| s.abs()).sum::<f32>() / bits.len().max(1) as f32;
    let k = if mean > 0.0 { 32.0 / mean } else { 0.0 };
    out.clear();
    out.extend(bits.iter().map(|&s| (s * k).clamp(-127.0, 127.0) as i8));
}

/// Bytes as bits, most significant first, appended to `out`.
pub fn bits_msb(b: &[u8], out: &mut Vec<u8>) {
    out.extend(
        b.iter()
            .flat_map(|v| (0..8).rev().map(move |i| (v >> i) & 1)),
    );
}

/// `n` bytes of the PN9 whitening (x^9 + x^5 + 1 from all ones), packed least significant bit first: FF E1 1D 9A ...
pub fn pn9_lsb(n: usize) -> Vec<u8> {
    let mut l = seq::PN9_WHITENING;
    let mut out = vec![0u8; n];
    l.fill_bytes(&mut out, BitOrder::Lsb);
    out
}

/// The 37 whitening bytes of Lucky-7: PN9 packed most significant bit first from its ninth output bit (87 B8 59 ...).
pub fn lucky7_whitening() -> [u8; LUCKY7_BYTES] {
    let mut l = seq::PN9_WHITENING;
    l.skip(8);
    let mut out = [0u8; LUCKY7_BYTES];
    l.fill_bytes(&mut out, BitOrder::Msb);
    out
}

// ---------------------------------------------------------------- Geoscan

/// The bytes after the Geoscan sync word for `data` (the frame length less 2): data and CRC-16/CMS, whitened.
pub fn geoscan_encode(data: &[u8]) -> Vec<u8> {
    let mut f = data.to_vec();
    f.extend_from_slice(&CRC16_CMS.checksum(data).to_be_bytes());
    for (b, w) in f.iter_mut().zip(pn9_lsb(data.len() + 2)) {
        *b ^= w;
    }
    f
}

/// Removes the whitening of a Geoscan frame in place and checks its CRC.
pub fn geoscan_decode(f: &mut [u8]) -> bool {
    if f.len() < 3 {
        return false;
    }
    let w = pn9_lsb(f.len());
    for (b, w) in f.iter_mut().zip(w) {
        *b ^= w;
    }
    let n = f.len() - 2;
    CRC16_CMS.checksum(&f[..n]) == u16::from_be_bytes([f[n], f[n + 1]])
}

/// The Geoscan framing: a frame of fixed length, its CRC over all bytes but the last two.
pub struct GeoscanFraming {
    /// Frame bytes after the sync word (66 or 74).
    pub len: usize,
    buf: Vec<u8>,
}

impl GeoscanFraming {
    /// The framing of `len`-byte frames.
    pub fn new(len: usize) -> Self {
        GeoscanFraming {
            len,
            buf: Vec::with_capacity(len),
        }
    }
}

impl Framing for GeoscanFraming {
    type Frame = Decoded;
    fn frame_bits(&mut self, _header: &[f32]) -> Option<usize> {
        Some(8 * self.len)
    }
    fn decode(&mut self, bits: &[f32]) -> Option<Decoded> {
        soft_bytes(bits, &mut self.buf);
        geoscan_decode(&mut self.buf).then(|| Decoded {
            data: self.buf.clone(),
            validated: "CRC-16",
            ..Decoded::default()
        })
    }
}

// ---------------------------------------------------------------- Lucky-7

/// The 39 bytes of a Lucky-7 frame of a 35-byte payload: the sync word and the whitened payload and CRC-16/CMS.
pub fn lucky7_encode(payload: &[u8; 35]) -> [u8; 39] {
    let mut f = [0u8; 39];
    f[..2].copy_from_slice(&LUCKY7_SYNC.to_be_bytes());
    f[2..37].copy_from_slice(payload);
    f[37..].copy_from_slice(&CRC16_CMS.checksum(payload).to_be_bytes());
    for (b, w) in f[2..].iter_mut().zip(lucky7_whitening()) {
        *b ^= w;
    }
    f
}

/// The 35-byte payload of the 37 whitened bytes after a Lucky-7 sync word, when the CRC passes.
pub fn lucky7_decode(f: &[u8]) -> Option<[u8; 35]> {
    if f.len() != LUCKY7_BYTES {
        return None;
    }
    let mut d = [0u8; LUCKY7_BYTES];
    for ((o, &b), w) in d.iter_mut().zip(f).zip(lucky7_whitening()) {
        *o = b ^ w;
    }
    let mut p = [0u8; 35];
    p.copy_from_slice(&d[..35]);
    (CRC16_CMS.checksum(&p) == u16::from_be_bytes([d[35], d[36]])).then_some(p)
}

/// The kind of a Lucky-7 payload, from its byte 1.
pub fn lucky7_kind(payload: &[u8]) -> &'static str {
    match payload.get(1).copied().unwrap_or(0) {
        0 => "telemetry",
        b if b & 0xC0 == 0xC0 => "picture",
        b if b & 0x20 != 0 => "dosimetry",
        _ => "other",
    }
}

// ---------------------------------------------------------------- Spino

/// The largest Spino size field taken.
pub const SPINO_MAX_SIZE: usize = 4096;

/// The bytes after the Spino sync word: the 16-byte header, the size, the body and CRC-16/XMODEM (least significant byte first).
pub fn spino_encode(header: &[u8; 16], body: &[u8]) -> Vec<u8> {
    let s = (body.len() + 4) as u16;
    let mut f = header.to_vec();
    f.extend_from_slice(&s.to_le_bytes());
    f.extend_from_slice(body);
    let crc = CRC16_XMODEM.checksum(&f);
    f.extend_from_slice(&crc.to_le_bytes());
    f
}

/// The Spino framing.
#[derive(Default)]
pub struct SpinoFraming {
    buf: Vec<u8>,
}

impl Framing for SpinoFraming {
    type Frame = Decoded;
    fn header_bits(&self) -> usize {
        18 * 8
    }
    fn frame_bits(&mut self, header: &[f32]) -> Option<usize> {
        soft_bytes(header, &mut self.buf);
        let s = usize::from(u16::from_le_bytes([self.buf[16], self.buf[17]]));
        (4..=SPINO_MAX_SIZE).contains(&s).then_some(8 * (s + 16))
    }
    fn decode(&mut self, bits: &[f32]) -> Option<Decoded> {
        soft_bytes(bits, &mut self.buf);
        let n = self.buf.len() - 2;
        let crc = u16::from_le_bytes([self.buf[n], self.buf[n + 1]]);
        (CRC16_XMODEM.checksum(&self.buf[..n]) == crc).then(|| Decoded {
            data: self.buf.clone(),
            validated: "CRC-16",
            ..Decoded::default()
        })
    }
}

// ---------------------------------------------------------------- CCSDS header ids

/// The spacecraft and virtual channel of a frame by its first two bytes: a TM header (version 0) or an AOS header (version 1).
pub fn header_ids(frame: &[u8]) -> Option<(u16, u8)> {
    let mut h = [0u8; 6];
    let n = frame.len().min(6);
    h[..n].copy_from_slice(&frame[..n]);
    match frame.first()? >> 6 {
        0 => tm::PrimaryHeader::parse(&h)
            .ok()
            .map(|t| (t.spacecraft, t.vcid)),
        1 => aos::PrimaryHeader::parse(&h)
            .ok()
            .map(|a| (u16::from(a.spacecraft), a.vcid)),
        _ => None,
    }
}

// ---------------------------------------------------------------- USP

/// The block of a USP frame: `d` data bytes (48 or 223) with the AX.25 frame `ax25` (without FCS) in it.
fn usp_block(d: usize, ax25: &[u8]) -> Vec<u8> {
    let mut b = vec![0u8; d];
    b[0] = 0x08;
    b[1] = 0xFF;
    b[2..4].copy_from_slice(&(ax25.len() as u16).to_le_bytes());
    b[4..4 + ax25.len()].copy_from_slice(ax25);
    b
}

/// The USP channel coding of a block of `d` data bytes: RS(d + 32, d) in the dual basis, randomised.
fn usp_channel(d: usize) -> ChannelConfig {
    ChannelConfig::rs_223(1).with_virtual_fill(223 - d)
}

/// The bits of a USP frame after its sync word: the PLS code and the coded block. `ax25` must fit in 219 bytes.
pub fn usp_encode(ax25: &[u8]) -> Vec<u8> {
    let d = if ax25.len() + 4 <= 48 { 48 } else { 223 };
    usp_encode_block(&usp_block(d, ax25))
}

/// The bits after the USP sync word of a block of 48 or 223 data bytes as given: the PLS code and the coded block.
pub fn usp_encode_block(block: &[u8]) -> Vec<u8> {
    let d = block.len();
    let mut coded = Vec::new();
    ChannelCoder::new(usp_channel(d))
        .expect("a CCSDS code")
        .encode(block, &mut coded)
        .expect("a block of the code's length");
    let mut bits = Vec::new();
    bits_msb(
        &if d == 48 { USP_PLS_48 } else { USP_PLS_223 }.to_be_bytes(),
        &mut bits,
    );
    let mut data = Vec::new();
    bits_msb(&coded[4..], &mut data);
    Encoder::new(codes::CCSDS_K7).encode(&data, &mut bits);
    bits
}

/// The USP framing.
pub struct UspFraming {
    d: usize,
    viterbi: Viterbi,
    llr: Vec<i8>,
    bits: Vec<u8>,
    bytes: Vec<u8>,
    rs: [ChannelDecoder; 2],
}

impl Default for UspFraming {
    fn default() -> Self {
        UspFraming {
            d: 0,
            viterbi: Viterbi::new(codes::CCSDS_K7, 8 * 255).expect("a K = 7 code"),
            llr: Vec::new(),
            bits: Vec::new(),
            bytes: Vec::new(),
            rs: [48, 223].map(|d| ChannelDecoder::new(usp_channel(d)).expect("a CCSDS code")),
        }
    }
}

impl Framing for UspFraming {
    type Frame = Decoded;
    fn header_bits(&self) -> usize {
        64
    }
    fn frame_bits(&mut self, header: &[f32]) -> Option<usize> {
        let corr = |code: u64| {
            header
                .iter()
                .enumerate()
                .map(|(i, &s)| if (code >> (63 - i)) & 1 == 0 { s } else { -s })
                .sum::<f32>()
        };
        self.d = if corr(USP_PLS_48) > corr(USP_PLS_223) {
            48
        } else {
            223
        };
        Some(64 + 16 * (self.d + 32))
    }
    fn decode(&mut self, bits: &[f32]) -> Option<Decoded> {
        // Several candidates may be open: the block size follows from the length, not from the last header read.
        self.d = (bits.len().checked_sub(64)? / 16).checked_sub(32)?;
        if self.d != 48 && self.d != 223 {
            return None;
        }
        llrs(&bits[64..], &mut self.llr);
        let n = 8 * (self.d + 32);
        self.bits.resize(n, 0);
        self.viterbi
            .decode_only(&self.llr, Ends::Start(0), &mut self.bits)
            .ok()?;
        self.bytes.clear();
        self.bytes.extend(
            self.bits
                .as_chunks::<8>()
                .0
                .iter()
                .map(|c| c.iter().fold(0u8, |a, &b| (a << 1) | b)),
        );
        let rs = &mut self.rs[usize::from(self.d == 223)];
        let r = rs.decode(&mut self.bytes).ok()?;
        if !r.ok() {
            return None;
        }
        let d = &self.bytes[..self.d];
        let len = usize::from(u16::from_le_bytes([d[2], d[3]]));
        if len == 0 || len + 4 > self.d {
            return None;
        }
        Some(Decoded {
            data: d[4..4 + len].to_vec(),
            rs_corrected: Some(r.corrected),
            validated: "Viterbi + Reed-Solomon",
            ..Decoded::default()
        })
    }
}

// ---------------------------------------------------------------- NGHam

/// The NGHam sizes: size tag, largest payload, data bytes, check bytes.
pub const NGHAM_SIZES: [(u32, usize, usize, usize); 7] = [
    (0x3B49CD, 28, 31, 16),
    (0x4DDA57, 60, 63, 16),
    (0x76939A, 92, 95, 16),
    (0x9BB4AE, 124, 127, 32),
    (0xA0FD63, 156, 159, 32),
    (0xD66EF9, 188, 191, 32),
    (0xED2734, 220, 223, 32),
];

/// The NGHam Reed-Solomon code of a code word of `n` bytes with `checks` check bytes.
fn ngham_code(n: usize, checks: usize) -> RsCode {
    let base = if checks == 16 {
        rs_codes::CCSDS_255_239_ROOT112_CONVENTIONAL
    } else {
        rs_codes::CCSDS_255_223_CONVENTIONAL
    };
    base.shortened(n as u16)
}

/// The smallest NGHam size whose largest payload holds `len` bytes.
pub fn ngham_size(len: usize) -> Option<usize> {
    NGHAM_SIZES.iter().position(|s| s.1 >= len)
}

/// The bytes after the NGHam sync word: the size tag and the randomised code word. With `rs` false the check bytes are
/// sent as zeros.
pub fn ngham_encode(payload: &[u8], flags: u8, rs: bool) -> Option<Vec<u8>> {
    let size = ngham_size(payload.len())?;
    let (tag, largest, data, checks) = NGHAM_SIZES[size];
    let mut w = vec![((flags & 7) << 5) | (largest - payload.len()) as u8];
    w.extend_from_slice(payload);
    let crc = CRC16_IBM_SDLC.checksum(&w);
    w.extend_from_slice(&crc.to_be_bytes());
    w.resize(data + checks, 0);
    if rs {
        Rs::new(ngham_code(data + checks, checks))
            .ok()?
            .encode_word(&mut w)
            .ok()?;
    }
    randomiser::tm_bytes(&mut w);
    let mut out = tag.to_be_bytes()[1..].to_vec();
    out.extend_from_slice(&w);
    Some(out)
}

/// The NGHam framing.
pub struct NghamFraming {
    size: usize,
    buf: Vec<u8>,
    rs: Vec<Rs>,
}

impl Default for NghamFraming {
    fn default() -> Self {
        NghamFraming {
            size: 0,
            buf: Vec::new(),
            rs: NGHAM_SIZES
                .iter()
                .map(|s| Rs::new(ngham_code(s.2 + s.3, s.3)).expect("an NGHam code"))
                .collect(),
        }
    }
}

impl NghamFraming {
    /// The payload and flags of a code word whose randomiser is removed, when its CRC passes.
    fn payload_of(&self, w: &[u8]) -> Option<(usize, u8)> {
        let largest = NGHAM_SIZES[self.size].1;
        let pad = usize::from(w[0] & 0x1F);
        let len = largest.checked_sub(pad)?;
        let crc = u16::from_be_bytes([w[1 + len], w[2 + len]]);
        (CRC16_IBM_SDLC.checksum(&w[..1 + len]) == crc).then_some((len, w[0] >> 5))
    }
}

impl Framing for NghamFraming {
    type Frame = Decoded;
    fn header_bits(&self) -> usize {
        24
    }
    fn frame_bits(&mut self, header: &[f32]) -> Option<usize> {
        let tag = header
            .iter()
            .fold(0u32, |a, &s| (a << 1) | u32::from(s < 0.0));
        let (size, d) = NGHAM_SIZES
            .iter()
            .enumerate()
            .map(|(i, s)| (i, (s.0 ^ tag).count_ones()))
            .min_by_key(|x| x.1)?;
        self.size = size;
        (d <= 6).then_some(24 + 8 * (NGHAM_SIZES[size].2 + NGHAM_SIZES[size].3))
    }
    fn decode(&mut self, bits: &[f32]) -> Option<Decoded> {
        self.size = NGHAM_SIZES
            .iter()
            .position(|s| 24 + 8 * (s.2 + s.3) == bits.len())?;
        soft_bytes(&bits[24..], &mut self.buf);
        randomiser::tm_bytes(&mut self.buf);
        let data = NGHAM_SIZES[self.size].2;
        let (found, corrected) = match self.payload_of(&self.buf) {
            Some(f) => {
                let none = self.buf[data..].iter().all(|&b| b == 0);
                (f, if none { -1 } else { 0 })
            }
            None => {
                let r = self.rs[self.size].decode(&mut self.buf, &[]).ok()?;
                (self.payload_of(&self.buf)?, r.corrected as i64)
            }
        };
        let (len, flags) = found;
        Some(Decoded {
            data: self.buf[1..1 + len].to_vec(),
            rs_corrected: u32::try_from(corrected).ok(),
            validated: if corrected < 0 {
                "CRC-16"
            } else {
                "Reed-Solomon + CRC-16"
            },
            flags: Some(flags),
            ..Decoded::default()
        })
    }
}

// ---------------------------------------------------------------- U482C

/// U482C length field flag: the convolutional code is used.
pub const U482C_VITERBI: u16 = 0x100;
/// U482C length field flag: the CCSDS randomiser is used.
pub const U482C_RANDOMISER: u16 = 0x200;
/// U482C length field flag: Reed-Solomon is used.
pub const U482C_RS: u16 = 0x400;

/// The largest U482C packet with these flags.
pub fn u482c_max_packet(flags: u16) -> usize {
    match (flags & U482C_VITERBI != 0, flags & U482C_RS != 0) {
        (true, true) => 94,
        (true, false) => 126,
        (false, true) => 223,
        (false, false) => 255,
    }
}

/// The length field and the bytes after the U482C sync word for `packet` with `flags` (the three flags above). `None` for a
/// packet that does not fit.
pub fn u482c_encode(packet: &[u8], flags: u16) -> Option<Vec<u8>> {
    if packet.is_empty() || packet.len() > u482c_max_packet(flags) {
        return None;
    }
    let mut w = packet.to_vec();
    if flags & U482C_RS != 0 {
        let n = w.len() + 32;
        w.resize(n, 0);
        Rs::new(rs_codes::CCSDS_255_223_CONVENTIONAL.shortened(n as u16))
            .ok()?
            .encode_word(&mut w)
            .ok()?;
    }
    if flags & U482C_RANDOMISER != 0 {
        randomiser::tm_bytes(&mut w);
    }
    let body = if flags & U482C_VITERBI != 0 {
        let mut bits = Vec::new();
        bits_msb(&w, &mut bits);
        let mut coded = Vec::new();
        let mut enc = Encoder::new(codes::INMARSAT_K7);
        enc.encode(&bits, &mut coded);
        enc.terminate(&mut coded);
        coded.extend([0u8; 4]);
        coded
            .chunks(8)
            .map(|c| c.iter().fold(0u8, |a, &b| (a << 1) | b))
            .collect()
    } else {
        w
    };
    let ax = Ax100::new().ok()?;
    let field = ax.golay_encode(body.len() as u16 | (flags & 0x700));
    let mut out = field.to_be_bytes()[1..].to_vec();
    out.extend_from_slice(&body);
    Some(out)
}

/// The U482C framing. A stream hunter takes only frames with Reed-Solomon (`rs_only`): without it a frame has no check.
pub struct U482cFraming {
    /// Take only frames with the Reed-Solomon flag and more than 32 bytes.
    pub rs_only: bool,
    flags: u16,
    len: usize,
    ax: Ax100,
    viterbi: Viterbi,
    llr: Vec<i8>,
    bits: Vec<u8>,
    buf: Vec<u8>,
    rs: Vec<Option<Rs>>,
}

impl Default for U482cFraming {
    fn default() -> Self {
        U482cFraming {
            rs_only: true,
            flags: 0,
            len: 0,
            ax: Ax100::new().expect("the Golay code"),
            viterbi: Viterbi::new(codes::INMARSAT_K7, 8 * 256).expect("a K = 7 code"),
            llr: Vec::new(),
            bits: Vec::new(),
            buf: Vec::new(),
            rs: vec![None; 256],
        }
    }
}

impl Framing for U482cFraming {
    type Frame = Decoded;
    fn header_bits(&self) -> usize {
        24
    }
    fn frame_bits(&mut self, header: &[f32]) -> Option<usize> {
        let field = header
            .iter()
            .fold(0u32, |a, &s| (a << 1) | u32::from(s < 0.0));
        let (v, _) = self.ax.golay_decode(field).ok()?;
        self.len = usize::from(v & 0xFF);
        self.flags = v & 0x700;
        if self.len == 0 || (self.rs_only && (self.flags & U482C_RS == 0 || self.len <= 32)) {
            return None;
        }
        if self.flags & U482C_VITERBI != 0 && (self.len < 4 || self.len % 2 == 1) {
            return None;
        }
        Some(24 + 8 * self.len)
    }
    fn decode(&mut self, bits: &[f32]) -> Option<Decoded> {
        // Read the field again: another candidate's header may have been read since.
        self.frame_bits(&bits[..24])?;
        let body = &bits[24..];
        let mut validated = "Golay";
        if self.flags & U482C_VITERBI != 0 {
            let n = self.len / 2 - 1;
            llrs(&body[..16 * n + 12], &mut self.llr);
            self.bits.resize(8 * n, 0);
            self.viterbi
                .decode_only(&self.llr, Ends::Zero, &mut self.bits)
                .ok()?;
            self.buf.clear();
            self.buf.extend(
                self.bits
                    .as_chunks::<8>()
                    .0
                    .iter()
                    .map(|c| c.iter().fold(0u8, |a, &b| (a << 1) | b)),
            );
            validated = "Golay + Viterbi";
        } else {
            soft_bytes(body, &mut self.buf);
        }
        if self.flags & U482C_RANDOMISER != 0 {
            randomiser::tm_bytes(&mut self.buf);
        }
        let mut corrected = None;
        if self.flags & U482C_RS != 0 {
            let n = self.buf.len();
            if !(33..=255).contains(&n) {
                return None;
            }
            let rs = self.rs[n].get_or_insert_with(|| {
                Rs::new(rs_codes::CCSDS_255_223_CONVENTIONAL.shortened(n as u16))
                    .expect("a shortened code")
            });
            corrected = Some(rs.decode(&mut self.buf, &[]).ok()?.corrected);
            self.buf.truncate(n - 32);
            validated = if self.flags & U482C_VITERBI != 0 {
                "Golay + Viterbi + Reed-Solomon"
            } else {
                "Golay + Reed-Solomon"
            };
        }
        Some(Decoded {
            data: self.buf.clone(),
            rs_corrected: corrected,
            validated,
            ..Decoded::default()
        })
    }
}

// ---------------------------------------------------------------- Mobitex

sdrkit_core::choice_enum! {
    /// The Mobitex variant of the Mobitex-NX sync word.
    pub enum MobitexVariant {
        /// Mobitex and Mobitex-NX: blocks counted by the control byte, a callsign; a frame whose callsign fails is read as
        /// BEESAT-1.
        Nx = ("mobitex_nx", "Mobitex / Mobitex-NX"),
        /// BEESAT-1: no callsign.
        Beesat1 = ("beesat1", "BEESAT-1"),
        /// BEESAT-9: always 32 blocks.
        Beesat9 = ("beesat9", "BEESAT-9"),
    }
}

/// The layout a Mobitex frame was read with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MobitexLayout {
    /// The plain Mobitex sync word.
    Mobitex,
    /// Mobitex-NX.
    Nx,
    /// BEESAT-1.
    Beesat1,
    /// BEESAT-9.
    Beesat9,
}

impl MobitexLayout {
    /// The name in the frame table.
    pub fn name(self) -> &'static str {
        match self {
            MobitexLayout::Mobitex => "Mobitex",
            MobitexLayout::Nx => "Mobitex-NX",
            MobitexLayout::Beesat1 => "BEESAT-1",
            MobitexLayout::Beesat9 => "BEESAT-9",
        }
    }
    fn callsign(self) -> bool {
        self != MobitexLayout::Beesat1
    }
}

/// Repairs a callsign and its CRC-16/XMODEM (8 bytes) with up to 2 flipped bits. Returns false when no repair passes or a
/// character is not printable.
pub fn mobitex_callsign(c: &mut [u8; 8]) -> bool {
    let ok = |c: &[u8; 8]| {
        CRC16_XMODEM.checksum(&c[..6]) == u16::from_be_bytes([c[6], c[7]])
            && c[..6].iter().all(|b| (0x20..=0x7E).contains(b))
    };
    if ok(c) {
        return true;
    }
    let flip = |c: &mut [u8; 8], i: usize| c[i / 8] ^= 0x80 >> (i % 8);
    for i in 0..64 {
        flip(c, i);
        if ok(c) {
            return true;
        }
        for j in i + 1..64 {
            flip(c, j);
            if ok(c) {
                return true;
            }
            flip(c, j);
        }
        flip(c, i);
    }
    false
}

/// The bits after a Mobitex sync word: control bytes `c0`, `c1`, their check byte, the callsign with its CRC (unless
/// `layout` is BEESAT-1) and the blocks of 18 bytes, each with its CRC, coded, interleaved and scrambled. The low 5 bits of
/// `c0` are set to the block count less 1.
pub fn mobitex_encode(
    layout: MobitexLayout,
    c0: u8,
    c1: u8,
    callsign: &str,
    blocks: &[[u8; 18]],
) -> Vec<u8> {
    let c0 = (c0 & 0xE0) | (blocks.len().clamp(1, 32) - 1) as u8;
    let mut head = vec![c0, c1, (mobitex::checks(c0) << 4) | mobitex::checks(c1)];
    if layout.callsign() {
        let mut call = [b' '; 6];
        for (o, c) in call.iter_mut().zip(callsign.bytes()) {
            *o = c;
        }
        head.extend_from_slice(&call);
        head.extend_from_slice(&CRC16_XMODEM.checksum(&call).to_be_bytes());
    }
    let mut bits = Vec::new();
    bits_msb(&head, &mut bits);
    let mut scr = mobitex::Scrambler::new();
    for b in blocks {
        let mut bytes = [0u8; 20];
        bytes[..18].copy_from_slice(b);
        bytes[18..].copy_from_slice(&CRC16_IBM_SDLC.checksum(b).to_be_bytes());
        let mut sent = [0u8; mobitex::BLOCK_BITS];
        mobitex::encode_block(&bytes, &mut sent);
        scr.apply(&mut sent);
        bits.extend_from_slice(&sent);
    }
    bits
}

/// The Mobitex framing of one sync word.
pub struct MobitexFraming {
    /// The plain Mobitex sync word (else Mobitex-NX).
    pub plain: bool,
    /// The variant of the Mobitex-NX sync word.
    pub variant: MobitexVariant,
    /// The least blocks whose CRC passes for a frame (1 unless set).
    pub min_good: usize,
    layout: MobitexLayout,
    head: Vec<u8>,
    blocks: usize,
}

impl MobitexFraming {
    /// The framing of the plain Mobitex word (`plain`) or of the Mobitex-NX word.
    pub fn new(plain: bool, variant: MobitexVariant) -> Self {
        MobitexFraming {
            plain,
            variant,
            min_good: 1,
            layout: MobitexLayout::Nx,
            head: Vec::new(),
            blocks: 0,
        }
    }

    fn header_len(&self) -> usize {
        if !self.plain && self.variant == MobitexVariant::Beesat1 {
            3
        } else {
            11
        }
    }
}

impl Framing for MobitexFraming {
    type Frame = Decoded;
    fn header_bits(&self) -> usize {
        8 * self.header_len()
    }
    fn frame_bits(&mut self, header: &[f32]) -> Option<usize> {
        soft_bytes(header, &mut self.head);
        let w0 = (u16::from(self.head[0]) << 4) | u16::from(self.head[2] >> 4);
        let w1 = (u16::from(self.head[1]) << 4) | u16::from(self.head[2] & 0xF);
        let (c0, _) = mobitex::decode(w0)?;
        let (c1, _) = mobitex::decode(w1)?;
        self.head[0] = c0;
        self.head[1] = c1;
        self.blocks = usize::from(c0 & 0x1F) + 1;
        self.layout = if self.plain {
            MobitexLayout::Mobitex
        } else {
            match self.variant {
                MobitexVariant::Nx => MobitexLayout::Nx,
                MobitexVariant::Beesat1 => MobitexLayout::Beesat1,
                MobitexVariant::Beesat9 => MobitexLayout::Beesat9,
            }
        };
        if self.layout == MobitexLayout::Beesat9 {
            self.blocks = 32;
        }
        let mut head_len = 3;
        if self.layout.callsign() {
            let mut call = [0u8; 8];
            call.copy_from_slice(&self.head[3..11]);
            if mobitex_callsign(&mut call) {
                self.head[3..11].copy_from_slice(&call);
                head_len = 11;
            } else if self.layout == MobitexLayout::Nx {
                self.layout = MobitexLayout::Beesat1;
            } else {
                return None;
            }
        }
        self.head.truncate(head_len);
        Some(8 * head_len + mobitex::BLOCK_BITS * self.blocks)
    }
    fn decode(&mut self, bits: &[f32]) -> Option<Decoded> {
        // Read the header again: another candidate's header may have been read since.
        let total = self.frame_bits(&bits[..self.header_bits()])?;
        if total != bits.len() {
            return None;
        }
        let head_len = self.head.len();
        let mut out = vec![self.head[0], self.head[1]];
        out.extend_from_slice(&self.head[3..]);
        let mut scr = mobitex::Scrambler::new();
        let mut map = 0u32;
        let mut good = 0;
        for (b, chunk) in bits[8 * head_len..]
            .as_chunks::<{ mobitex::BLOCK_BITS }>()
            .0
            .iter()
            .enumerate()
        {
            let mut sent = [0u8; mobitex::BLOCK_BITS];
            for (o, &s) in sent.iter_mut().zip(chunk) {
                *o = u8::from(s < 0.0);
            }
            scr.apply(&mut sent);
            let mut bytes = [0u8; 20];
            mobitex::decode_block(&sent, &mut bytes);
            let crc = u16::from_be_bytes([bytes[18], bytes[19]]);
            if CRC16_IBM_SDLC.checksum(&bytes[..18]) == crc {
                good += 1;
            } else {
                map |= 1 << b;
            }
            out.extend_from_slice(&bytes[..18]);
        }
        if good < self.min_good.max(1) {
            return None;
        }
        out.push(0xAA);
        out.extend_from_slice(&map.to_le_bytes());
        out.push(0xBB);
        let callsign = self
            .layout
            .callsign()
            .then(|| String::from_utf8_lossy(&self.head[3..9]).replace(' ', ""));
        Some(Decoded {
            data: out,
            validated: match (good, self.blocks) {
                (g, n) if g == n => "FEC + CRC-16",
                _ => "FEC + CRC-16 (some blocks failed)",
            },
            kind: Some(self.layout.name()),
            callsign,
            rs_corrected: None,
            flags: None,
        })
    }
}

// ---------------------------------------------------------------- CCSDS concatenated

sdrkit_core::choice_enum! {
    /// The K = 7 code of a CCSDS concatenated link: the order of the two outputs and the inversion of G2.
    pub enum ConcatCode {
        /// G1 first, G2 inverted.
        Ccsds = ("ccsds", "CCSDS"),
        /// G2 inverted first, G1 second.
        NasaDsn = ("nasa_dsn", "NASA-DSN"),
        /// G1 first, G2 not inverted.
        CcsdsUninverted = ("ccsds_uninverted", "CCSDS uninverted"),
        /// G2 first, G1 second, no inversion.
        NasaDsnUninverted = ("nasa_dsn_uninverted", "NASA-DSN uninverted"),
    }
}

impl ConcatCode {
    /// The code.
    pub fn code(self) -> ConvCode {
        match self {
            ConcatCode::Ccsds => codes::CCSDS_K7,
            ConcatCode::NasaDsn => codes::NASA_DSN_K7,
            ConcatCode::CcsdsUninverted => codes::K7_171_133,
            ConcatCode::NasaDsnUninverted => codes::INMARSAT_K7,
        }
    }
}

/// The rules of a CCSDS concatenated link.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ConcatConfig {
    /// Data bytes of a frame, a multiple of the interleave.
    pub frame_bytes: usize,
    /// Interleave, 1 to 8.
    pub interleave: usize,
    /// The Reed-Solomon basis.
    pub basis: Basis,
    /// The K = 7 code.
    pub code: ConcatCode,
    /// NRZ-M before the code.
    pub nrzm: bool,
    /// The randomiser.
    pub randomiser: bool,
}

impl Default for ConcatConfig {
    fn default() -> Self {
        ConcatConfig {
            frame_bytes: 223,
            interleave: 1,
            basis: Basis::Dual,
            code: ConcatCode::Ccsds,
            nrzm: false,
            randomiser: true,
        }
    }
}

impl ConcatConfig {
    /// The channel coding of the block, or `None` when the sizes do not fit.
    pub fn channel(&self) -> Option<ChannelConfig> {
        let i = self.interleave;
        if !(1..=8).contains(&i)
            || self.frame_bytes == 0
            || !self.frame_bytes.is_multiple_of(i)
            || self.frame_bytes / i > 223
        {
            return None;
        }
        let mut c = ChannelConfig::rs_223(i)
            .with_virtual_fill(223 - self.frame_bytes / i)
            .with_basis(BasisPolicy::Fixed(self.basis));
        if !self.randomiser {
            c = c.without_randomiser();
        }
        Some(c)
    }

    /// Bytes of the block after the sync word.
    pub fn block_bytes(&self) -> usize {
        self.frame_bytes + 32 * self.interleave
    }
}

/// The code symbols of a CCSDS concatenated frame of `frame` (exactly `cfg.frame_bytes` bytes), with 8 bytes of 55 before the
/// sync word and 4 after the block.
pub fn concat_encode(cfg: &ConcatConfig, frame: &[u8]) -> Option<Vec<u8>> {
    let mut coded = Vec::new();
    ChannelCoder::new(cfg.channel()?)
        .ok()?
        .encode(frame, &mut coded)
        .ok()?;
    let mut bits = Vec::new();
    bits_msb(&[0x55; 8], &mut bits);
    bits_msb(&coded, &mut bits);
    bits_msb(&[0x55; 4], &mut bits);
    if cfg.nrzm {
        let mut level = 0u8;
        for b in &mut bits {
            level ^= *b;
            *b = level;
        }
    }
    let mut out = Vec::new();
    Encoder::new(cfg.code.code()).encode(&bits, &mut out);
    Some(out)
}

/// The framing of the decoded bits of a CCSDS concatenated link: the block behind the sync word, every code word decoded.
pub struct ConcatFraming {
    rs: ChannelDecoder,
    frame_bytes: usize,
    block_bytes: usize,
    buf: Vec<u8>,
}

impl ConcatFraming {
    /// The framing of `cfg`, or `None` when its sizes do not fit.
    pub fn new(cfg: &ConcatConfig) -> Option<Self> {
        Some(ConcatFraming {
            rs: ChannelDecoder::new(cfg.channel()?).ok()?,
            frame_bytes: cfg.frame_bytes,
            block_bytes: cfg.block_bytes(),
            buf: Vec::new(),
        })
    }
}

impl Framing for ConcatFraming {
    type Frame = Decoded;
    fn frame_bits(&mut self, _header: &[f32]) -> Option<usize> {
        Some(8 * self.block_bytes)
    }
    fn decode(&mut self, bits: &[f32]) -> Option<Decoded> {
        soft_bytes(bits, &mut self.buf);
        let r = self.rs.decode(&mut self.buf).ok()?;
        r.ok().then(|| Decoded {
            data: self.buf[..self.frame_bytes].to_vec(),
            rs_corrected: Some(r.corrected),
            validated: "Viterbi + Reed-Solomon",
            ..Decoded::default()
        })
    }
}

/// The framing of a CCSDS Reed-Solomon frame of fixed length: randomised, one RS(255,223) word shortened to `len` bytes, in
/// one basis.
pub struct CcsdsRsFraming {
    rs: ChannelDecoder,
    len: usize,
    buf: Vec<u8>,
}

impl CcsdsRsFraming {
    /// The framing of `len`-byte code words (33 to 255) in `basis`.
    pub fn new(len: usize, basis: Basis) -> Option<Self> {
        let cfg = ChannelConfig::rs_223(1)
            .with_virtual_fill(255usize.checked_sub(len)?)
            .with_basis(BasisPolicy::Fixed(basis));
        Some(CcsdsRsFraming {
            rs: ChannelDecoder::new(cfg).ok()?,
            len,
            buf: Vec::new(),
        })
    }
}

impl Framing for CcsdsRsFraming {
    type Frame = Decoded;
    fn frame_bits(&mut self, _header: &[f32]) -> Option<usize> {
        Some(8 * self.len)
    }
    fn decode(&mut self, bits: &[f32]) -> Option<Decoded> {
        soft_bytes(bits, &mut self.buf);
        let r = self.rs.decode(&mut self.buf).ok()?;
        r.ok().then(|| Decoded {
            data: self.buf[..self.len - 32].to_vec(),
            rs_corrected: Some(r.corrected),
            validated: "Reed-Solomon",
            ..Decoded::default()
        })
    }
}

/// The marker of the CCSDS links.
pub const ASM: u32 = 0x1ACF_FC1D;

const _: () = assert!(marker::TM.len_bits() == 32);

#[cfg(test)]
mod tests {
    use super::*;
    use sdrkit_proto_link::hunt::{HuntConfig, Hunter};

    fn hex(s: &str) -> Vec<u8> {
        s.split_whitespace()
            .map(|h| u8::from_str_radix(h, 16).unwrap())
            .collect()
    }

    fn soft(bits: &[u8]) -> Vec<f32> {
        bits.iter()
            .map(|&b| if b == 1 { -1.0 } else { 1.0 })
            .collect()
    }

    #[test]
    fn whitening_sequences() {
        assert_eq!(
            pn9_lsb(16),
            hex("FF E1 1D 9A ED 85 33 24 EA 7A D2 39 70 97 57 0A")
        );
        let mut l = seq::PN9_WHITENING;
        let mut msb = [0u8; 16];
        l.fill_bytes(&mut msb, BitOrder::Msb);
        assert_eq!(
            msb.to_vec(),
            hex("FF 87 B8 59 B7 A1 CC 24 57 5E 4B 9C 0E E9 EA 50")
        );
        assert_eq!(
            lucky7_whitening().to_vec(),
            hex("87 B8 59 B7 A1 CC 24 57 5E 4B 9C 0E E9 EA 50 2A BE B4 1B B6 B0 5D F1 E6 9A E3 45 FD 2C 53 18 0C CA C9 FB 49 37")
        );
    }

    #[test]
    fn catalogue_check_values() {
        assert_eq!(CRC16_CMS.checksum(b"123456789"), 0xAEE7);
        assert_eq!(CRC16_IBM_SDLC.checksum(b"123456789"), 0x906E);
        assert_eq!(CRC16_XMODEM.checksum(b"123456789"), 0x31C3);
        assert_eq!(
            sdrkit_util_bits::crc::catalog::CRC24_LTE_A.checksum(b"123456789"),
            0xCDE703
        );
    }

    #[test]
    fn geoscan_vector() {
        let data: Vec<u8> = (0..64).collect();
        assert_eq!(CRC16_CMS.checksum(&data), 0x4329);
        let f = geoscan_encode(&data);
        assert_eq!(f.len(), 66);
        assert_eq!(
            f[..16],
            hex("FF E0 1F 99 E9 80 35 23 E2 73 D8 32 7C 9A 59 05")[..]
        );
        assert_eq!(f[62..], hex("E1 BC BC D9")[..]);
        let mut g = f.clone();
        assert!(geoscan_decode(&mut g));
        assert_eq!(g[..64], data[..]);
        for bit in [0, 100, 527] {
            let mut g = f.clone();
            g[bit / 8] ^= 0x80 >> (bit % 8);
            assert!(!geoscan_decode(&mut g));
        }
    }

    #[test]
    fn lucky7_vector_and_kinds() {
        let mut p = [0u8; 35];
        for (i, b) in p.iter_mut().enumerate().skip(2) {
            *b = (i - 1) as u8;
        }
        assert_eq!(CRC16_CMS.checksum(&p), 0x6177);
        let f = lucky7_encode(&p);
        assert_eq!(
            f.to_vec(),
            hex("2D D4 87 B8 58 B5 A2 C8 21 51 59 43 95 04 E2 E6 5D 24 B1 A4 0A A4 A3 49 E4 F0 8D FB 5C E7 37 4F 05 12 D5 E9 DA 28 40")
        );
        assert_eq!(lucky7_decode(&f[2..]), Some(p));
        assert_eq!(lucky7_kind(&p), "telemetry");
        for (b, k) in [
            (0x00, "telemetry"),
            (0xC5, "picture"),
            (0x25, "dosimetry"),
            (0x11, "other"),
        ] {
            assert_eq!(lucky7_kind(&[0, b]), k);
        }
    }

    fn hunt<F: Framing<Frame = Decoded>>(
        word: u128,
        word_bits: u32,
        errors: u32,
        framing: F,
        bits: &[u8],
    ) -> Vec<Decoded> {
        let cfg = HuntConfig {
            word,
            word_bits,
            max_errors: errors,
            max_frame_bits: 16384,
            ..HuntConfig::default()
        };
        let mut h = Hunter::new(cfg, framing).unwrap();
        let mut out = Vec::new();
        for s in soft(bits) {
            h.push(s, &mut out);
        }
        out.into_iter().map(|f| f.frame).collect()
    }

    fn with_sync(word: u64, n: u32, after: &[u8]) -> Vec<u8> {
        let mut bits = vec![0, 1, 0, 1, 0, 1, 0, 1];
        bits.extend((0..n).rev().map(|k| ((word >> k) & 1) as u8));
        bits_msb(after, &mut bits);
        bits.extend([0u8; 16]);
        bits
    }

    #[test]
    fn spino_vector_and_limits() {
        let f = spino_encode(&[0xAA; 16], b"SPINO");
        let mut want = vec![0xAA; 16];
        want.extend(hex("09 00 53 50 49 4E 4F FC 74"));
        assert_eq!(f, want);
        let got = hunt(
            SPINO_SYNC as u128,
            32,
            0,
            SpinoFraming::default(),
            &with_sync(SPINO_SYNC as u64, 32, &f),
        );
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].data, want);
        let mut big = f.clone();
        big[16..18].copy_from_slice(&4097u16.to_le_bytes());
        assert!(hunt(
            SPINO_SYNC as u128,
            32,
            0,
            SpinoFraming::default(),
            &with_sync(SPINO_SYNC as u64, 32, &big)
        )
        .is_empty());
        assert!(hunt(
            SPINO_SYNC as u128,
            32,
            0,
            SpinoFraming::default(),
            &with_sync(SPINO_SYNC as u64 ^ 0x100, 32, &f)
        )
        .is_empty());
    }

    #[test]
    fn ngham_vector() {
        let f = ngham_encode(b"HELLO", 0, true).unwrap();
        assert_eq!(
            f,
            hex("3B 49 CD E8 00 4B 8C D6 42 CC 50 8E 2C 93 AD A7 B7 46 CE 5A 97 7D CC 32 A2 BF 3E 0A 10 F1 88 94 CD EA 39 EC 55 8B 4C 07 9C E4 AC 19 F7 D9 51 83 F4 FB")
        );
        for (len, size) in [(28, 0), (29, 1), (60, 1), (61, 2), (220, 6)] {
            assert_eq!(ngham_size(len), Some(size));
        }
        let run = |bytes: &[u8]| {
            hunt(
                NGHAM_SYNC as u128,
                32,
                4,
                NghamFraming::default(),
                &with_sync(NGHAM_SYNC as u64, 32, bytes),
            )
        };
        let got = run(&f);
        assert_eq!(got.len(), 1);
        assert_eq!(
            (got[0].data.as_slice(), got[0].rs_corrected),
            (&b"HELLO"[..], Some(0))
        );
        let mut bad = f.clone();
        for i in 0..8 {
            bad[3 + i * 5] ^= 0x33;
        }
        assert_eq!(run(&bad)[0].rs_corrected, Some(8));
        bad[3 + 41] ^= 0x33;
        assert!(run(&bad).is_empty());
        let plain = ngham_encode(b"HELLO", 0, false).unwrap();
        let got = run(&plain);
        assert_eq!(
            (got[0].data.as_slice(), got[0].rs_corrected),
            (&b"HELLO"[..], None)
        );
    }

    #[test]
    fn usp_frames_round_trip_and_choose_their_block() {
        let ax = |n: usize| -> Vec<u8> {
            let mut f = hex("A4 64 82 9C 8C 40 60 A4 A6 60 60 A6 40 6F 03 F0");
            f.extend((0..n).map(|i| b'A' + (i % 26) as u8));
            f
        };
        for (n, d) in [(11, 48), (74, 223)] {
            let bits = usp_encode(&ax(n));
            assert_eq!(bits.len(), 64 + 16 * (d + 32));
            let mut s = with_sync(USP_SYNC, 64, &[]);
            s.truncate(s.len() - 16);
            s.extend(&bits);
            s.extend([0u8; 16]);
            let got = hunt(USP_SYNC as u128, 64, 13, UspFraming::default(), &s);
            assert_eq!(got.len(), 1, "{n}");
            assert_eq!(got[0].data, ax(n));
        }
        // A length field of 0, or one that runs past the block, is refused.
        for len in [0u16, 45] {
            let mut b = usp_block(48, &ax(4));
            b[2..4].copy_from_slice(&len.to_le_bytes());
            let mut s = with_sync(USP_SYNC, 64, &[]);
            s.truncate(s.len() - 16);
            s.extend(usp_encode_block(&b));
            s.extend([0u8; 16]);
            assert!(
                hunt(USP_SYNC as u128, 64, 13, UspFraming::default(), &s).is_empty(),
                "{len}"
            );
        }
    }

    #[test]
    fn u482c_lengths_and_every_flag_combination() {
        let packet: Vec<u8> = (0..40).collect();
        let f = u482c_encode(&packet, U482C_RS | U482C_RANDOMISER).unwrap();
        let ax = Ax100::new().unwrap();
        let field = u32::from_be_bytes([0, f[0], f[1], f[2]]);
        assert_eq!(ax.golay_decode(field).unwrap().0, 0x648);
        let f = u482c_encode(&packet, U482C_RS | U482C_RANDOMISER | U482C_VITERBI).unwrap();
        let field = u32::from_be_bytes([0, f[0], f[1], f[2]]);
        assert_eq!(ax.golay_decode(field).unwrap().0, 0x792);
        assert_eq!(f.len(), 3 + 146);
        let big: Vec<u8> = (0..94).collect();
        let f = u482c_encode(&big, U482C_RS | U482C_VITERBI).unwrap();
        assert_eq!(f.len() - 3, 254);
        assert!(u482c_encode(&(0..95).collect::<Vec<u8>>(), U482C_RS | U482C_VITERBI).is_none());
        for flags in 0..8u16 {
            let flags = flags << 8;
            let f = u482c_encode(&packet, flags).unwrap();
            let fr = U482cFraming {
                rs_only: false,
                ..U482cFraming::default()
            };
            let got = hunt(
                U482C_SYNC as u128,
                32,
                4,
                fr,
                &with_sync(U482C_SYNC as u64, 32, &f),
            );
            assert_eq!(got.len(), 1, "{flags:X}");
            assert_eq!(got[0].data, packet, "{flags:X}");
        }
        // Two Golay bit errors and 16 byte errors after the code still decode.
        let mut f = u482c_encode(&packet, U482C_RS | U482C_RANDOMISER).unwrap();
        f[0] ^= 0x81;
        for i in 0..16 {
            f[3 + 4 * i] ^= 0x5A;
        }
        let got = hunt(
            U482C_SYNC as u128,
            32,
            4,
            U482cFraming::default(),
            &with_sync(U482C_SYNC as u64, 32, &f),
        );
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].data, packet);
        assert_eq!(got[0].rs_corrected, Some(16));
    }

    #[test]
    fn mobitex_header_and_round_trip() {
        // The header of the BEESAT-9 test packet: control 3F 02, check C6, callsign DP0BEM and its CRC 4DF7.
        assert_eq!(CRC16_XMODEM.checksum(b"DP0BEM"), 0x4DF7);
        let blocks: Vec<[u8; 18]> = (0..32u8).map(|b| [b; 18]).collect();
        let bits = mobitex_encode(MobitexLayout::Beesat9, 0x20, 0x02, "DP0BEM", &blocks);
        let mut head = Vec::new();
        for c in bits[..88].chunks(8) {
            head.push(c.iter().fold(0u8, |a, &b| (a << 1) | b));
        }
        assert_eq!(head, hex("3F 02 C6 44 50 30 42 45 4D 4D F7"));
        let mut s = vec![0, 1, 0, 1];
        s.extend((0..16).rev().map(|k| ((MOBITEX_NX_SYNC >> k) & 1) as u8));
        s.extend(&bits);
        s.extend([0u8; 16]);
        let got = hunt(
            MOBITEX_NX_SYNC as u128,
            16,
            1,
            MobitexFraming::new(false, MobitexVariant::Beesat9),
            &s,
        );
        assert_eq!(got.len(), 1);
        let d = &got[0].data;
        assert_eq!(d.len(), 592);
        assert_eq!(d[d.len() - 6..], [0xAA, 0, 0, 0, 0, 0xBB]);
        assert_eq!(got[0].callsign.as_deref(), Some("DP0BEM"));
        // Four blocks, control 20 01, through the default variant.
        let blocks: Vec<[u8; 18]> = (0..4u8).map(|b| [b + 1; 18]).collect();
        let bits = mobitex_encode(MobitexLayout::Nx, 0x20, 0x01, "DP0TST", &blocks);
        let mut s = vec![0, 1];
        s.extend((0..16).rev().map(|k| ((MOBITEX_NX_SYNC >> k) & 1) as u8));
        s.extend(&bits);
        s.extend([0u8; 16]);
        let got = hunt(
            MOBITEX_NX_SYNC as u128,
            16,
            1,
            MobitexFraming::new(false, MobitexVariant::Nx),
            &s,
        );
        assert_eq!(got[0].kind, Some("Mobitex-NX"));
        assert_eq!(got[0].data.len(), 2 + 8 + 4 * 18 + 6);
    }

    #[test]
    fn concat_every_code_basis_and_nrzm_round_trips_in_both_polarities() {
        assert_eq!(
            ConcatConfig {
                frame_bytes: 892,
                interleave: 4,
                ..ConcatConfig::default()
            }
            .block_bytes(),
            1020
        );
        let frame: Vec<u8> = (0..223).map(|i| (i * 7) as u8).collect();
        let syms = concat_encode(&ConcatConfig::default(), &frame).unwrap();
        assert_eq!(syms.len(), 2 * 8 * (8 + 4 + 255 + 4));
        for code in [
            ConcatCode::Ccsds,
            ConcatCode::NasaDsn,
            ConcatCode::CcsdsUninverted,
            ConcatCode::NasaDsnUninverted,
        ] {
            for basis in [Basis::Dual, Basis::Conventional] {
                for nrzm in [false, true] {
                    let cfg = ConcatConfig {
                        code,
                        basis,
                        nrzm,
                        ..ConcatConfig::default()
                    };
                    for invert in [false, true] {
                        let syms = concat_encode(&cfg, &frame).unwrap();
                        let llr: Vec<i8> = syms
                            .iter()
                            .map(|&c| if (c == 1) ^ invert { -40 } else { 40 })
                            .collect();
                        let mut v = Viterbi::new(code.code(), llr.len() / 2).unwrap();
                        let mut bits = vec![0u8; llr.len() / 2];
                        v.decode_only(&llr, Ends::Open, &mut bits).unwrap();
                        if nrzm {
                            let mut prev = 0;
                            for b in &mut bits {
                                let d = *b ^ prev;
                                prev = *b;
                                *b = d;
                            }
                        }
                        let got =
                            hunt(ASM as u128, 32, 4, ConcatFraming::new(&cfg).unwrap(), &bits);
                        assert_eq!(got.len(), 1, "{code:?} {basis:?} {nrzm} {invert}");
                        assert_eq!(got[0].data, frame);
                    }
                }
            }
        }
    }

    #[test]
    fn header_rule() {
        assert_eq!(header_ids(&[0x44, 0x85]), Some((18, 5)));
    }
}
