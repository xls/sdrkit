//! M17: link setup frames, stream frames with LICH, packet frames, the end of transmission, callsign addresses, the TYPE
//! field, META text and GNSS, SMS packets, and the Codec 2 frames. The payload of an encrypted stream never leaves.
//! Standard: M17 specification 2.0.

use crate::bits::{bits_to_bytes, bytes_to_bits};
use crate::conv::ConvChannel;
use crate::decoder::{Ctx, ModeParams, Protocol};
use crate::gen::{bits_to_levels, silence, FskModulator, Prepared, TxPulse};
use crate::items::*;
use crate::symbols::{Sym, SymAnswer, SyncAnswer, SyncPattern};
use crate::typed::M17Fields;
use sdrkit_algo_fec::conv::codes::{M17_K5, M17_P1, M17_P2, M17_P3};
use sdrkit_algo_fec::small::{Chase, SmallCode};
use sdrkit_core::interface::Fields;
use sdrkit_core::media::Codec;
use sdrkit_core::publish::Published;
use sdrkit_core::publish::Declare;
use sdrkit_core::{Complex32, GenConfig, Value};
use sdrkit_util_bits::crc::catalog::CRC16_M17 as CRC;
use sdrkit_util_bits::interleave::Permutation;
use sdrkit_util_bits::pack::{push_u64, to_u64};
use sdrkit_util_bits::seq::M17_RANDOMISER;
use sdrkit_util_text::base40;
use std::sync::LazyLock;

pub const SYNC_LSF: &str = "55F7";
pub const SYNC_STREAM: &str = "FF5D";
pub const SYNC_PACKET: &str = "75FF";
pub const SYNC_BERT: &str = "DF55";
pub const SYNC_EOT: &str = "555D";

/// Extended Golay (24,12) of the LICH words.
static LICH_WORD: LazyLock<SmallCode> = LazyLock::new(SmallCode::golay_24_12);
/// The bit interleaver: coded bit `i` is sent at `(45 i + 92 i^2) mod 368` (M17 specification, "Interleaving").
static INTERLEAVE: LazyLock<Permutation> =
    LazyLock::new(|| Permutation::qpp(45, 92, 368).expect("a permutation of 368"));

/// Encodes a callsign as a 48-bit address ("@ALL" is the broadcast address). A character outside the M17 alphabet returns the
/// empty address.
pub fn encode_callsign(cs: &str) -> u64 {
    base40::m17_encode(&cs.to_ascii_uppercase()).unwrap_or(0)
}

/// Decodes a 48-bit address (empty for 0 and for an address outside the defined ranges).
pub fn decode_callsign(v: u64) -> String {
    let mut s = String::new();
    if v != 0 && base40::m17_decode_into(v, &mut s).is_err() {
        s.clear();
    }
    s.trim_end().to_string()
}

/// The decoders an M17 receiver keeps.
#[derive(Clone, Debug)]
pub struct Codecs {
    /// The link setup frame: 244 input bits, puncture P1.
    pub lsf: ConvChannel,
    /// The stream frame payload: 148 input bits, puncture P2.
    pub stream: ConvChannel,
    /// The packet frame: 210 input bits, puncture P3.
    pub packet: ConvChannel,
}

impl Default for Codecs {
    fn default() -> Self {
        Codecs {
            lsf: ConvChannel::new(M17_K5, Some(M17_P1), 244),
            stream: ConvChannel::new(M17_K5, Some(M17_P2), 148),
            packet: ConvChannel::new(M17_K5, Some(M17_P3), 210),
        }
    }
}

/// Bit `i` of the randomiser.
fn randomiser_bit(i: usize) -> u8 {
    (M17_RANDOMISER.bytes()[i / 8] >> (7 - i % 8)) & 1
}

/// Interleaves and randomizes 368 coded bits.
pub fn scramble(bits: &[u8]) -> Vec<u8> {
    let mut out = vec![0u8; 368];
    if bits.len() < 368 || INTERLEAVE.apply(&bits[..368], &mut out).is_err() {
        return out;
    }
    for (i, b) in out.iter_mut().enumerate() {
        *b ^= randomiser_bit(i);
    }
    out
}

/// The inverse of [`scramble`], on soft values (positive means 1).
pub fn unscramble_soft(soft: &[f32]) -> Vec<f32> {
    let mut s = vec![0f32; 368];
    for (i, v) in s.iter_mut().enumerate() {
        let x = soft.get(i).copied().unwrap_or(0.0);
        *v = if randomiser_bit(i) != 0 { -x } else { x };
    }
    let mut out = vec![0f32; 368];
    if INTERLEAVE.invert(&s, &mut out).is_err() {
        out.fill(0.0);
    }
    out
}

/// Decodes one LICH word from its 24 soft values (positive means 1) with a Chase search over the least reliable bits. Returns
/// the 12 information bits and the bits changed, or `None` when no trial produces a code word.
pub fn decode_lich_word(soft: &[f32]) -> Option<(u16, u32)> {
    if soft.len() < 24 {
        return None;
    }
    let hard = soft[..24]
        .iter()
        .fold(0u128, |a, &v| (a << 1) | (v > 0.0) as u128);
    let rel: [f32; 24] = std::array::from_fn(|i| soft[i].abs());
    let (cw, changed, _) = LICH_WORD.decode_soft(hard, &rel, Chase::default())?;
    Some((LICH_WORD.info(cw) as u16, changed as u32))
}

/// Builds the 240-bit LSF with its CRC.
pub fn lsf_bytes(dst: u64, src: u64, typ: u16, meta: &[u8; 14]) -> [u8; 30] {
    let mut b = [0u8; 30];
    b[..6].copy_from_slice(&dst.to_be_bytes()[2..]);
    b[6..12].copy_from_slice(&src.to_be_bytes()[2..]);
    b[12..14].copy_from_slice(&typ.to_be_bytes());
    b[14..28].copy_from_slice(meta);
    let crc = CRC.checksum(&b[..28]);
    b[28..].copy_from_slice(&crc.to_be_bytes());
    b
}

/// The coded LSF (368 bits, before interleaving).
pub fn encode_lsf(lsf: &[u8; 30]) -> Vec<u8> {
    let mut bits = bytes_to_bits(lsf);
    bits.extend([0, 0, 0, 0]);
    let mut out = Vec::new();
    Codecs::default().lsf.encode(&bits, &mut out);
    out
}

/// The fields of the TYPE word.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TypeField {
    pub stream: bool,
    /// 1 data, 2 voice, 3 voice and data.
    pub data_type: u8,
    /// 0 none, 1 scrambler, 2 AES, 3 other.
    pub enc_type: u8,
    pub enc_subtype: u8,
    pub can: u8,
    pub signed: bool,
}

impl TypeField {
    pub fn decode(t: u16) -> TypeField {
        TypeField {
            stream: t & 1 != 0,
            data_type: ((t >> 1) & 3) as u8,
            enc_type: ((t >> 3) & 3) as u8,
            enc_subtype: ((t >> 5) & 3) as u8,
            can: ((t >> 7) & 0xF) as u8,
            signed: (t >> 11) & 1 != 0,
        }
    }
    pub fn encode(&self) -> u16 {
        (self.stream as u16)
            | ((self.data_type as u16 & 3) << 1)
            | ((self.enc_type as u16 & 3) << 3)
            | ((self.enc_subtype as u16 & 3) << 5)
            | ((self.can as u16 & 0xF) << 7)
            | ((self.signed as u16) << 11)
    }
    pub fn kind(&self) -> &'static str {
        if !self.stream {
            return "packet";
        }
        match self.data_type {
            1 => "data",
            2 => "voice 3200",
            3 => "voice + data 1600",
            _ => "stream",
        }
    }
    pub fn encryption(&self) -> &'static str {
        match (self.enc_type, self.enc_subtype) {
            (0, _) => "",
            (1, 0) => "scrambler 8-bit",
            (1, 1) => "scrambler 16-bit",
            (1, _) => "scrambler 24-bit",
            (2, 0) => "AES-128",
            (2, 1) => "AES-192",
            (2, _) => "AES-256",
            _ => "other",
        }
    }
}

#[derive(Clone, Debug, Default)]
struct Stream {
    lsf: Option<[u8; 30]>,
    lich: [Option<[u8; 5]>; 6],
    meta_text: [Option<Vec<u8>>; 4],
    data: Vec<u8>,
    last_fn: Option<u16>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Frame {
    Lsf,
    Stream,
    Packet,
    None,
}

/// The M17 protocol.
pub struct M17 {
    frame: Frame,
    bits: Vec<f32>,
    stream: Stream,
    packet: Vec<u8>,
    packet_lsf: Option<[u8; 30]>,
    lsf_ok: u64,
    stream_frames: u64,
    packets_ok: u64,
    idle: usize,
    last_lsf: Option<[u8; 30]>,
    since_sync: usize,
    locked: bool,
    sync_q: f32,
    codecs: Codecs,
}

impl Default for M17 {
    fn default() -> Self {
        M17 {
            frame: Frame::None,
            bits: Vec::with_capacity(368),
            stream: Stream::default(),
            packet: Vec::new(),
            packet_lsf: None,
            lsf_ok: 0,
            stream_frames: 0,
            packets_ok: 0,
            idle: 0,
            last_lsf: None,
            since_sync: 1000,
            locked: false,
            sync_q: 1.0,
            codecs: Codecs::default(),
        }
    }
}

const KEY: u32 = 1;

impl M17 {
    fn lsf_fields(lsf: &[u8; 30]) -> (String, String, TypeField) {
        let dst = u64::from_be_bytes([0, 0, lsf[0], lsf[1], lsf[2], lsf[3], lsf[4], lsf[5]]);
        let src = u64::from_be_bytes([0, 0, lsf[6], lsf[7], lsf[8], lsf[9], lsf[10], lsf[11]]);
        let t = TypeField::decode(u16::from_be_bytes([lsf[12], lsf[13]]));
        (decode_callsign(src), decode_callsign(dst), t)
    }

    fn decode_lsf(&mut self, soft: &[f32], ctx: &mut Ctx) {
        let d = unscramble_soft(soft);
        let (b, errs): ([u8; 30], usize) = match self.codecs.lsf.decode(&d, 244, true) {
            Ok((bits, errs)) => (
                bits_to_bytes(&bits[..240]).try_into().unwrap_or([0; 30]),
                errs,
            ),
            Err(_) => ([0; 30], 0),
        };
        let ok = CRC.verify(&b, sdrkit_util_bits::crc::Trailer::Big);
        ctx.identity(ok);
        ctx.block(ok, 240, errs);
        self.locked = ok;
        if !ok {
            return;
        }
        self.lsf_ok += 1;
        self.last_lsf = Some(b);
        let (_, _, t) = Self::lsf_fields(&b);
        if t.stream {
            self.start_stream(b, false, ctx);
        } else {
            self.packet_lsf = Some(b);
            self.packet.clear();
        }
    }

    fn start_stream(&mut self, lsf: [u8; 30], from_lich: bool, ctx: &mut Ctx) {
        if self.stream.lsf == Some(lsf) && ctx.has_call(KEY) {
            return;
        }
        if ctx.has_call(KEY) {
            ctx.end_call(KEY, false);
        }
        let (src, dst, t) = Self::lsf_fields(&lsf);
        self.stream = Stream {
            lsf: Some(lsf),
            ..Default::default()
        };
        let enc = if t.enc_type == 0 {
            Encryption::clear()
        } else {
            Encryption::encrypted(
                Some(t.enc_type as u32 * 4 + t.enc_subtype as u32),
                t.encryption(),
                None,
            )
        };
        let mut rec = CallRecord {
            kind: CallKind::Voice,
            src: Ident::Text(src),
            dst: Ident::Text(dst.clone()),
            group: dst == "@ALL" || dst.starts_with('#'),
            access: Some(Access::new(AccessKind::Can, u32::from(t.can))),
            encryption: enc.clone(),
            ..Default::default()
        };
        if t.data_type == 1 {
            rec.kind = CallKind::Data;
        }
        rec.set(CallDetail::StreamType, Value::Text(t.kind().into()));
        rec.set(
            CallDetail::EncryptionType,
            Value::Text(t.encryption().into()),
        );
        rec.set(CallDetail::LsfFromLich, Value::Bool(from_lich));
        rec.set(CallDetail::Signed, Value::Bool(t.signed));
        ctx.start_call(KEY, rec, from_lich);
        ctx.set_encryption(KEY, enc);
        self.meta(&lsf, ctx);
    }

    /// META: text (blocks of 13 characters), a GNSS position or extended callsign data, by the TYPE's META type (the encryption
    /// subtype when there is no encryption).
    fn meta(&mut self, lsf: &[u8; 30], ctx: &mut Ctx) {
        let (src, dst, t) = Self::lsf_fields(lsf);
        let m = &lsf[14..28];
        if t.enc_type != 0 || m.iter().all(|&b| b == 0) {
            return;
        }
        if t.enc_subtype == 1 {
            if let Some(mut p) = gnss(m) {
                p.src = Ident::Text(src.clone());
                p.time = ctx.now;
                let at = Value::Position {
                    lat: p.lat,
                    lon: p.lon,
                };
                ctx.update_call(KEY, |c| c.set(CallDetail::Position, at));
                ctx.position(p);
            }
            return;
        }
        if t.enc_subtype != 0 {
            return;
        }
        // Text: control byte (block count in the high nibble as a mask, block index in the low nibble), 13 characters.
        let ctl = m[0];
        let idx = match ctl & 0x0F {
            1 => 0,
            2 => 1,
            4 => 2,
            8 => 3,
            _ => return,
        };
        self.stream.meta_text[idx] = Some(m[1..].to_vec());
        let mask = ctl >> 4;
        let complete = (0..4).all(|i| mask & (1 << i) == 0 || self.stream.meta_text[i].is_some());
        if complete {
            let mut s = Vec::new();
            for (i, part) in self.stream.meta_text.iter().enumerate() {
                if let Some(part) = part.as_ref().filter(|_| mask & (1 << i) != 0) {
                    s.extend_from_slice(part);
                }
            }
            let text = String::from_utf8_lossy(&s)
                .trim_end_matches(['\0', ' '])
                .to_string();
            let known = ctx.call(KEY).map(|c| c.text == text).unwrap_or(false);
            if !known && !text.is_empty() {
                ctx.update_call(KEY, |c| c.text = text.clone());
                ctx.message(MessageRecord {
                    src: Ident::Text(src),
                    dst: Ident::Text(dst),
                    group: false,
                    format: MessageFormat::MetaText,
                    data_format: None,
                    text,
                    crc_ok: true,
                    time: ctx.now,
                    slot: 0,
                    sms: false,
                });
            }
        }
    }

    fn decode_stream(&mut self, soft: &[f32], ctx: &mut Ctx) {
        let d = unscramble_soft(soft);
        // LICH: four Golay (24,12) words.
        let mut lich = Vec::with_capacity(48);
        let mut lich_ok = true;
        let mut lich_errs = 0;
        for k in 0..4 {
            match decode_lich_word(&d[k * 24..k * 24 + 24]) {
                Some((v, e)) => {
                    lich_errs += e as usize;
                    push_u64(&mut lich, v as u64, 12);
                }
                None => {
                    lich_ok = false;
                    push_u64(&mut lich, 0, 12);
                }
            }
        }
        let (payload, errs) = match self.codecs.stream.decode(&d[96..], 148, true) {
            Ok((bits, errs)) => (bits_to_bytes(&bits[..144]), errs),
            Err(_) => (vec![0u8; 18], usize::MAX / 2),
        };
        // A stream frame has no check of its own: the LICH words and the path metric must both look like a signal (random
        // bits pass a Golay word's check one time in two).
        let ok = lich_ok && lich_errs <= 3 && errs < 30;
        self.locked = ok;
        ctx.identity(ok);
        if ctx.has_call(KEY) || ok {
            ctx.block(ok, 240, (lich_errs + errs).min(240));
        }
        self.stream_frames += 1;
        if ok {
            let lb = bits_to_bytes(&lich);
            let cnt = (lb[5] >> 5) as usize;
            if cnt < 6 {
                let mut chunk = [0u8; 5];
                chunk.copy_from_slice(&lb[..5]);
                self.stream.lich[cnt] = Some(chunk);
                // Late entry: the LSF from six LICH chunks.
                if self.stream.lsf.is_none() || !ctx.has_call(KEY) {
                    if self.stream.lich.iter().all(|c| c.is_some()) {
                        let mut l = [0u8; 30];
                        for (d, c) in l
                            .as_chunks_mut::<5>()
                            .0
                            .iter_mut()
                            .zip(self.stream.lich.iter().flatten())
                        {
                            *d = *c;
                        }
                        if CRC.verify(&l, sdrkit_util_bits::crc::Trailer::Big) {
                            self.lsf_ok += 1;
                            self.start_stream(l, true, ctx);
                        }
                    }
                } else if let Some(lsf) = self.stream.lsf {
                    // Each complete cycle of six LICH chunks carries the META again (it can rotate): read it.
                    if cnt == 5 && self.stream.lich.iter().all(|c| c.is_some()) {
                        let mut l = [0u8; 30];
                        for (d, c) in l
                            .as_chunks_mut::<5>()
                            .0
                            .iter_mut()
                            .zip(self.stream.lich.iter().flatten())
                        {
                            *d = *c;
                        }
                        if CRC.verify(&l, sdrkit_util_bits::crc::Trailer::Big)
                            && l[..14] == lsf[..14]
                        {
                            self.meta(&l, ctx);
                        }
                        self.stream.lich = Default::default();
                    }
                }
            }
        }
        let fnum = u16::from_be_bytes([payload[0], payload[1]]);
        let last = fnum & 0x8000 != 0;
        if !ctx.has_call(KEY) {
            return;
        }
        self.stream.last_fn = Some(fnum & 0x7FFF);
        let t = self
            .stream
            .lsf
            .map(|l| Self::lsf_fields(&l).2)
            .unwrap_or_default();
        ctx.update_call(KEY, |c| c.voice_frames += 1);
        match t.data_type {
            2 => {
                for (i, half) in payload[2..18].chunks(8).enumerate() {
                    let spec = crate::voice::FrameSpec {
                        codec: Codec::Codec2,
                        packaging: sdrkit_graph::SpeechPackaging::Raw,
                        codec_mode: Some(sdrkit_graph::Codec2Mode::M3200),
                        duration_ms: 20,
                        stack: "m17",
                        carrier_hz: ctx.carrier_hz,
                        slot: 0,
                    };
                    ctx.speech(KEY, spec, half, ctx.now + i as f64 * 0.02, !ok, None);
                }
            }
            3 => {
                let spec = crate::voice::FrameSpec {
                    codec: Codec::Codec2,
                    packaging: sdrkit_graph::SpeechPackaging::Raw,
                    codec_mode: Some(sdrkit_graph::Codec2Mode::M1600),
                    duration_ms: 40,
                    stack: "m17",
                    carrier_hz: ctx.carrier_hz,
                    slot: 0,
                };
                ctx.speech(KEY, spec, &payload[2..10], ctx.now, !ok, None);
                if t.enc_type == 0 {
                    self.stream.data.extend_from_slice(&payload[10..18]);
                }
            }
            1 if t.enc_type == 0 => self.stream.data.extend_from_slice(&payload[2..18]),
            _ => {}
        }
        if last {
            self.end_stream(true, ctx);
        }
    }

    fn end_stream(&mut self, end_seen: bool, ctx: &mut Ctx) {
        if !self.stream.data.is_empty() {
            let text: String = self
                .stream
                .data
                .iter()
                .filter(|b| b.is_ascii_graphic() || **b == b' ')
                .map(|&b| b as char)
                .collect();
            let text = text.chars().take(512).collect::<String>();
            ctx.update_call(KEY, |c| c.set(CallDetail::Data, Value::Text(text)));
        }
        ctx.update_call(KEY, |c| {
            c.set(CallDetail::Frames, Value::I64(i64::from(c.voice_frames)))
        });
        ctx.end_call(KEY, end_seen);
        self.stream = Stream::default();
    }

    fn decode_packet(&mut self, soft: &[f32], ctx: &mut Ctx) {
        let d = unscramble_soft(soft);
        let (b, errs) = match self.codecs.packet.decode(&d, 210, true) {
            Ok((bits, errs)) => (bits_to_bytes(&bits[..206]), errs),
            Err(_) => (vec![0u8; 26], usize::MAX / 2),
        };
        let last = b[25] & 0x80 != 0;
        let n = ((b[25] >> 2) & 0x1F) as usize;
        let ok = errs < 30;
        self.locked = errs < 15;
        ctx.block(ok, 200, errs.min(200));
        if !ok {
            self.packet.clear();
            return;
        }
        if last {
            self.packet.extend_from_slice(&b[..n.min(25)]);
            let p = std::mem::take(&mut self.packet);
            self.finish_packet(&p, ctx);
        } else {
            self.packet.extend_from_slice(&b[..25]);
            if self.packet.len() > 33 * 25 {
                self.packet.clear();
            }
        }
    }

    fn finish_packet(&mut self, p: &[u8], ctx: &mut Ctx) {
        if p.len() < 3 {
            return;
        }
        let crc_ok = CRC.verify(p, sdrkit_util_bits::crc::Trailer::Big);
        ctx.block(crc_ok, p.len() * 8, 0);
        let Some(lsf) = self.packet_lsf.or(self.last_lsf) else {
            return;
        };
        if !crc_ok {
            return;
        }
        self.packets_ok += 1;
        let (src, dst, t) = Self::lsf_fields(&lsf);
        let proto = p[0];
        let body = &p[1..p.len() - 2];
        let text = String::from_utf8_lossy(body)
            .trim_end_matches('\0')
            .to_string();
        let mut rec = CallRecord {
            kind: CallKind::Packet,
            src: Ident::Text(src.clone()),
            dst: Ident::Text(dst.clone()),
            access: Some(Access::new(AccessKind::Can, u32::from(t.can))),
            text: if proto == 0x05 {
                text.clone()
            } else {
                String::new()
            },
            voice_frames: 0,
            ..Default::default()
        };
        rec.set(CallDetail::StreamType, Value::Text("packet".into()));
        rec.set(CallDetail::Protocol, Value::I64(i64::from(proto)));
        rec.set(
            CallDetail::Frames,
            Value::I64(i64::from((p.len() as u32).div_ceil(25))),
        );
        if proto == 0x05 {
            ctx.message(MessageRecord {
                src: Ident::Text(src),
                dst: Ident::Text(dst),
                group: false,
                format: MessageFormat::Sms,
                data_format: None,
                text,
                crc_ok,
                time: ctx.now,
                slot: 0,
                sms: true,
            });
        }
        ctx.log_single(rec);
        self.packet_lsf = None;
    }
}

/// A GNSS META block (M17 2.0): data source (4 bits) and station type (4 bits). Validity flags and the bearing's top bit. The
/// bearing's low byte. Latitude and longitude as signed 24-bit fractions of 90 and 180 degrees. Altitude (16 bits, 0.5 m, from
/// -500 m). Speed (12 bits, 0.5 km/h). Four reserved bits and a reserved byte.
pub fn gnss(m: &[u8]) -> Option<PositionRecord> {
    let station = m[0] & 0x0F;
    let bearing = (((m[1] & 1) as u16) << 8) | m[2] as u16;
    let lat_raw = i32::from_be_bytes([m[3], m[4], m[5], 0]) >> 8;
    let lon_raw = i32::from_be_bytes([m[6], m[7], m[8], 0]) >> 8;
    let lat = lat_raw as f64 * 90.0 / 8_388_607.0;
    let lon = lon_raw as f64 * 180.0 / 8_388_607.0;
    if !(-90.0..=90.0).contains(&lat) {
        return None;
    }
    let alt = u16::from_be_bytes([m[9], m[10]]) as f64 * 0.5 - 500.0;
    let speed = (u16::from_be_bytes([m[11], m[12]]) >> 4) as f64 * 0.5;
    Some(PositionRecord {
        lat,
        lon,
        alt_m: Some(alt),
        speed_kmh: Some(speed),
        course_deg: Some(bearing as f64),
        source: PositionFormat::M17Gnss,
        station: Some(match station {
            0 => StationType::Fixed,
            1 => StationType::Mobile,
            2 => StationType::Handheld,
            _ => StationType::Other,
        }),
        data_source: Some(m[0] >> 4),
        ..Default::default()
    })
}

/// Encodes a GNSS META block (the inverse of [`gnss`]).
pub fn gnss_meta(
    lat: f64,
    lon: f64,
    alt_m: f64,
    speed_kmh: f64,
    bearing: u16,
    station: u8,
) -> [u8; 14] {
    let mut m = [0u8; 14];
    m[0] = 0xF0 | (station & 0x0F);
    m[1] = 0xE4 | ((bearing >> 8) as u8 & 1);
    m[2] = bearing as u8;
    let la = ((lat / 90.0 * 8_388_607.0).round() as i32).to_be_bytes();
    let lo = ((lon / 180.0 * 8_388_607.0).round() as i32).to_be_bytes();
    m[3..6].copy_from_slice(&la[1..]);
    m[6..9].copy_from_slice(&lo[1..]);
    m[9..11].copy_from_slice(&(((alt_m + 500.0) * 2.0).round() as u16).to_be_bytes());
    m[11..13].copy_from_slice(&((((speed_kmh * 2.0).round() as u16) & 0xFFF) << 4).to_be_bytes());
    m
}

impl Protocol for M17 {
    fn params(&self) -> ModeParams {
        ModeParams {
            mode: "m17",
            name: "M17",
            baud: 4800.0,
            two_level: false,
            channel_hz: 12_500.0,
            calls: &crate::results::CALLS_300,
            message_cap: 100,
            polarity_rule: true,
            clock_loop: true,
            extra_baud: None,
            stream_rate: crate::front::RATE,
        }
    }

    fn patterns(&self) -> Vec<SyncPattern> {
        vec![
            SyncPattern::from_hex_dibits(0, SYNC_LSF, 0)
                .with_max_err(0)
                .no_auto_invert(),
            SyncPattern::from_hex_dibits(1, SYNC_STREAM, 0)
                .with_max_err(0)
                .no_auto_invert(),
            SyncPattern::from_hex_dibits(2, SYNC_PACKET, 0)
                .with_max_err(0)
                .no_auto_invert(),
            SyncPattern::from_hex_dibits(3, SYNC_EOT, 0)
                .with_max_err(0)
                .no_auto_invert(),
        ]
    }

    fn on_sync(
        &mut self,
        id: u8,
        _inv: bool,
        pre: &[Sym],
        _at: u64,
        tracking: bool,
        ctx: &mut Ctx,
    ) -> SyncAnswer {
        // Eight symbols also turn up inside frames: a sync less than a frame after the last one is taken only when it is much
        // cleaner than the one that started the frame being read (rms distance of the sync symbols from their levels).
        let q = (pre.iter().map(|s| (s.v - s.d as f32).powi(2)).sum::<f32>()
            / pre.len().max(1) as f32)
            .sqrt();
        if tracking && self.since_sync < 189 && (self.locked || q > 0.5 * self.sync_q) {
            return SyncAnswer::Ignore;
        }
        self.sync_q = q;
        self.since_sync = 0;
        self.bits.clear();
        self.idle = 0;
        self.frame = match id {
            0 => Frame::Lsf,
            1 => Frame::Stream,
            2 => Frame::Packet,
            _ => {
                if ctx.has_call(KEY) {
                    self.end_stream(true, ctx);
                }
                self.locked = false;
                Frame::None
            }
        };
        SyncAnswer::Take
    }

    fn on_symbol(&mut self, s: Sym, _at: u64, ctx: &mut Ctx) -> SymAnswer {
        self.since_sync += 1;
        if self.frame == Frame::None {
            self.idle += 1;
            return if self.idle > 400 {
                SymAnswer::Stop
            } else {
                SymAnswer::Continue
            };
        }
        let (a, b) = s.soft_dibit();
        self.bits.push(a);
        self.bits.push(b);
        if self.bits.len() == 368 {
            if crate::symbols::carrier_gone(&[s])
                && self.bits.iter().filter(|v| v.abs() < 0.2).count() > 100
            {
                self.frame = Frame::None;
                return SymAnswer::Stop;
            }
            let soft = std::mem::take(&mut self.bits);
            match self.frame {
                Frame::Lsf => self.decode_lsf(&soft, ctx),
                Frame::Stream => self.decode_stream(&soft, ctx),
                Frame::Packet => self.decode_packet(&soft, ctx),
                Frame::None => {}
            }
            self.bits = soft;
            self.bits.clear();
            self.frame = Frame::None;
        }
        SymAnswer::Continue
    }

    fn on_tick(&mut self, ctx: &mut Ctx) {
        if ctx.has_call(KEY) && ctx.now - ctx.last_frame_s > 1.0 {
            self.end_stream(false, ctx);
        }
    }

    fn fields(&self) -> Vec<&'static dyn Declare> {
        <M17Fields as Fields>::SHAPE.fields.to_vec()
    }

    fn publish(&self, r: &mut Published) {
        r.put(&M17Fields {
            lsf_ok: self.lsf_ok as i64,
            stream_frames: self.stream_frames as i64,
            packets_ok: self.packets_ok as i64,
        });
    }

    fn codec(&self) -> Codec {
        Codec::Codec2
    }
}

// ---------------------------------------------------------------------------------------------------------------------------
// Frame builders, shared by the test signal and the sender.

/// The deviation of one symbol level, Hz: +3 and -3 are 2.4 kHz from the carrier, +1 and -1 0.8 kHz.
pub const HZ_PER_LEVEL: f64 = 800.0;
/// The bits of one frame: 8 symbols of sync and 184 of payload.
pub const FRAME_BITS: usize = 384;
/// The most packet frames of one packet: the 5-bit counter of 32 frames, then the last frame.
pub const PACKET_FRAMES_MAX: usize = 33;
/// The payload bytes of one packet frame.
pub const PACKET_CHUNK: usize = 25;

/// The 4FSK modulator of M17 at 48 kHz: 4800 symbols/s, root raised cosine with roll-off 0.5, [`HZ_PER_LEVEL`].
pub fn modulator() -> FskModulator {
    FskModulator::new(4800.0, TxPulse::RootRaisedCosine(0.5), HZ_PER_LEVEL)
}

/// The bits of one frame: the 16-bit sync word `sync`, then the 368 coded bits interleaved and randomised.
pub fn frame_bits(sync: &str, coded: &[u8]) -> Vec<u8> {
    let mut b = Vec::with_capacity(FRAME_BITS);
    push_u64(
        &mut b,
        u64::from_str_radix(sync, 16).expect("a constant hex word"),
        16,
    );
    b.extend(scramble(coded));
    b
}

/// The preamble of 192 symbols (40 ms): before a link setup frame (`lsf` true) +3, -3 in turn, else -3, +3.
pub fn preamble(lsf: bool) -> Vec<u8> {
    let mut b = Vec::with_capacity(FRAME_BITS);
    for _ in 0..FRAME_BITS / 8 {
        push_u64(&mut b, if lsf { 0x77 } else { 0xEE }, 8);
    }
    b
}

/// The end of transmission: the EOT word repeated over 192 symbols.
pub fn eot() -> Vec<u8> {
    let mut b = Vec::with_capacity(FRAME_BITS);
    for _ in 0..24 {
        push_u64(&mut b, 0x555D, 16);
    }
    b.truncate(FRAME_BITS);
    b
}

/// The 384 bits of the link setup frame `lsf`.
pub fn lsf_frame(lsf: &[u8; 30]) -> Vec<u8> {
    frame_bits(SYNC_LSF, &encode_lsf(lsf))
}

/// The 384 bits of stream frame `index` of a transmission: LICH chunk `index mod 6` of `lsf` (four Golay words), the frame
/// number `index mod 2^15` with the end-of-stream bit when `last`, and the 16 payload bytes, coded with puncture P2.
pub fn stream_frame(
    codecs: &Codecs,
    lsf: &[u8; 30],
    index: usize,
    last: bool,
    payload: &[u8; 16],
) -> Vec<u8> {
    let cnt = index % 6;
    let mut lich = Vec::with_capacity(48);
    for &b in &lsf[cnt * 5..cnt * 5 + 5] {
        push_u64(&mut lich, b as u64, 8);
    }
    push_u64(&mut lich, ((cnt as u64) << 5) & 0xFF, 8);
    let mut coded = Vec::with_capacity(368);
    for k in 0..4 {
        push_u64(
            &mut coded,
            LICH_WORD.encode(to_u64(&lich[k * 12..k * 12 + 12]) as u128) as u64,
            24,
        );
    }
    let fnum = (index % 0x8000) as u16 | if last { 0x8000 } else { 0 };
    let mut bytes = [0u8; 18];
    bytes[..2].copy_from_slice(&fnum.to_be_bytes());
    bytes[2..].copy_from_slice(payload);
    let mut bits = bytes_to_bits(&bytes);
    bits.extend([0, 0, 0, 0]);
    codecs.stream.encode(&bits, &mut coded);
    frame_bits(SYNC_STREAM, &coded)
}

/// A packet: the protocol byte, `body`, and the CRC-16 of both, high byte first.
pub fn packet_payload(protocol: u8, body: &[u8]) -> Vec<u8> {
    let mut p = Vec::with_capacity(body.len() + 3);
    p.push(protocol);
    p.extend_from_slice(body);
    let crc = CRC.checksum(&p);
    p.extend_from_slice(&crc.to_be_bytes());
    p
}

/// The SMS packet of `text`: protocol 0x05, the text, a zero byte and the CRC.
pub fn sms_payload(text: &str) -> Vec<u8> {
    let mut body = Vec::with_capacity(text.len() + 1);
    body.extend_from_slice(text.as_bytes());
    body.push(0);
    packet_payload(0x05, &body)
}

/// The 384-bit packet frames that carry `payload`, [`PACKET_CHUNK`] bytes each. Each frame ends in 6 bits: the end flag and
/// the bytes used in the last frame, else the frame counter. Coded with puncture P3.
pub fn packet_frames(codecs: &Codecs, payload: &[u8]) -> Vec<Vec<u8>> {
    let chunks: Vec<&[u8]> = payload.chunks(PACKET_CHUNK).collect();
    let mut out = Vec::with_capacity(chunks.len());
    for (i, c) in chunks.iter().enumerate() {
        let mut b = [0u8; 26];
        b[..c.len()].copy_from_slice(c);
        let last = i == chunks.len() - 1;
        b[25] = if last {
            0x80 | ((c.len() as u8) << 2)
        } else {
            ((i % 32) as u8) << 2
        };
        let mut bits = bytes_to_bits(&b);
        bits.truncate(206);
        bits.extend([0, 0, 0, 0]);
        let mut coded = Vec::with_capacity(368);
        codecs.packet.encode(&bits, &mut coded);
        coded.truncate(368);
        out.push(frame_bits(SYNC_PACKET, &coded));
    }
    out
}

// ---------------------------------------------------------------------------------------------------------------------------
// Generator.

/// A stream transmission: (src, dst, TYPE, META blocks rotating, frames, payload source).
pub struct StreamTx {
    pub src: String,
    pub dst: String,
    pub typ: TypeField,
    pub metas: Vec<[u8; 14]>,
    pub frames: usize,
    pub data: Vec<u8>,
}

fn stream_tx(tx: &StreamTx, rng: &mut sdrkit_algo_sim::rng::Rng) -> Vec<u8> {
    let codecs = Codecs::default();
    let mut out = preamble(true);
    let first = lsf_bytes(
        encode_callsign(&tx.dst),
        encode_callsign(&tx.src),
        tx.typ.encode(),
        &tx.metas[0],
    );
    out.extend(lsf_frame(&first));
    let mut data_pos = 0;
    for f in 0..tx.frames {
        let meta = &tx.metas[(f / 6) % tx.metas.len()];
        let lsf = lsf_bytes(
            encode_callsign(&tx.dst),
            encode_callsign(&tx.src),
            tx.typ.encode(),
            meta,
        );
        let mut payload = [0u8; 16];
        for (i, p) in payload.iter_mut().enumerate() {
            *p = if tx.typ.data_type == 3 && i >= 8 && !tx.data.is_empty() {
                data_pos += 1;
                tx.data[(data_pos - 1) % tx.data.len()]
            } else {
                rng.next_u32() as u8
            };
        }
        out.extend(stream_frame(&codecs, &lsf, f, f == tx.frames - 1, &payload));
    }
    out.extend(eot());
    out
}

fn packet_tx(src: &str, dst: &str, can: u8, sms: &str) -> Vec<u8> {
    let mut out = preamble(true);
    let typ = TypeField {
        stream: false,
        data_type: 1,
        can,
        ..Default::default()
    };
    let lsf = lsf_bytes(
        encode_callsign(dst),
        encode_callsign(src),
        typ.encode(),
        &[0; 14],
    );
    out.extend(lsf_frame(&lsf));
    for f in packet_frames(&Codecs::default(), &sms_payload(sms)) {
        out.extend(f);
    }
    out.extend(eot());
    out
}

/// META text blocks for a text of up to 52 characters.
pub fn text_metas(text: &str) -> Vec<[u8; 14]> {
    let b = text.as_bytes();
    let n = b.len().div_ceil(13).clamp(1, 4);
    let mask = (1u8 << n) - 1;
    (0..n)
        .map(|i| {
            let mut m = [b' '; 14];
            m[0] = (mask << 4) | (1 << i);
            for (k, &c) in b.iter().skip(i * 13).take(13).enumerate() {
                m[1 + k] = c;
            }
            m
        })
        .collect()
}

/// The M17 test signal.
pub fn render(seed: u64) -> Vec<Complex32> {
    let mut rng = sdrkit_algo_sim::rng::Rng::new(seed);
    let mut m = modulator();
    let mut out = Vec::new();
    silence(0.3, &mut out);
    let mut send = |bits: Vec<u8>, out: &mut Vec<Complex32>, gap: f64| {
        m.symbols(&bits_to_levels(&bits), out);
        m.flush(8, out);
        m.reset();
        silence(gap, out);
    };
    let t1 = StreamTx {
        src: "N0CALL".into(),
        dst: "@ALL".into(),
        typ: TypeField {
            stream: true,
            data_type: 2,
            ..Default::default()
        },
        metas: text_metas("Test signal one, 73"),
        frames: 75,
        data: vec![],
    };
    send(stream_tx(&t1, &mut rng), &mut out, 0.6);
    send(
        packet_tx("N0CALL", "TEST1", 0, "Hello from the test signal"),
        &mut out,
        0.6,
    );
    let t3 = StreamTx {
        src: "N0CALL/P".into(),
        dst: "ECHO".into(),
        typ: TypeField {
            stream: true,
            data_type: 3,
            enc_subtype: 1,
            can: 3,
            ..Default::default()
        },
        metas: vec![gnss_meta(48.2085, 16.3725, 180.0, 36.0, 90, 1)],
        frames: 60,
        data: b"M17 data stream from the test signal. ".to_vec(),
    };
    send(stream_tx(&t3, &mut rng), &mut out, 0.6);
    let t4 = StreamTx {
        src: "N0CALL".into(),
        dst: "@ALL".into(),
        typ: TypeField {
            stream: true,
            data_type: 2,
            enc_type: 2,
            enc_subtype: 2,
            can: 5,
            ..Default::default()
        },
        metas: vec![[0x11; 14]],
        frames: 30,
        data: vec![],
    };
    send(stream_tx(&t4, &mut rng), &mut out, 0.6);
    out
}

/// The M17 generator from the settings of the stack.
pub fn generator(cfg: &GenConfig) -> Prepared {
    Prepared::new(render(cfg.seed), cfg.rate_hz, true).with_info(12_500.0, 4800.0, 9600.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn callsigns_round_trip() {
        for c in ["N0CALL", "N0CALL/P", "ECHO", "TEST1", "@ALL"] {
            assert_eq!(decode_callsign(encode_callsign(c)), c);
        }
    }

    #[test]
    fn puncturing_returns_368_bits() {
        let c = Codecs::default();
        assert_eq!(c.lsf.coded_len(244), 368);
        assert_eq!(c.stream.coded_len(148), 272);
        assert_eq!(c.packet.coded_len(210), 368);
    }

    #[test]
    fn a_lich_word_with_four_weak_errors_decodes_with_its_soft_values() {
        let info = 0xA5C;
        let cw = LICH_WORD.encode(info);
        let mut soft: [f32; 24] =
            std::array::from_fn(|i| if (cw >> (23 - i)) & 1 != 0 { 1.0 } else { -1.0 });
        // Four errors on the weakest bits: beyond the 3 errors a hard decision corrects.
        for i in [2, 9, 15, 21] {
            soft[i] *= -0.1;
        }
        let hard = soft
            .iter()
            .fold(0u128, |a, &v| (a << 1) | (v > 0.0) as u128);
        assert_ne!(LICH_WORD.decode(hard).map(|(c, _)| c), Some(cw));
        assert_eq!(decode_lich_word(&soft), Some((info as u16, 4)));
    }

    #[test]
    fn gnss_round_trips() {
        let m = gnss_meta(48.2085, 16.3725, 180.0, 36.0, 90, 1);
        let p = gnss(&m).unwrap();
        assert!((p.lat - 48.2085).abs() < 1e-4 && (p.lon - 16.3725).abs() < 1e-4);
        assert_eq!(
            (p.alt_m, p.speed_kmh, p.course_deg),
            (Some(180.0), Some(36.0), Some(90.0))
        );
        // The block of the recorded test signal.
        let v = [
            0xF1, 0xE4, 0x5A, 0x44, 0x90, 0x2D, 0x0B, 0xA4, 0x86, 0x05, 0x50, 0x04, 0x80, 0x00,
        ];
        let p = gnss(&v).unwrap();
        assert!((p.lat - 48.2085).abs() < 1e-3 && (p.lon - 16.3725).abs() < 1e-3);
        assert_eq!(
            (p.alt_m, p.speed_kmh, p.course_deg),
            (Some(180.0), Some(36.0), Some(90.0))
        );
    }
}
