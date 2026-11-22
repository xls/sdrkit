//! D-STAR digital voice: GMSK at 4800 bit/s, the radio header, voice frames with slow data (text, header copy, GPS in NMEA
//! and D-PRS, code squelch), data sync and the end pattern, and the AMBE voice frames.
//! Standard: JARL D-STAR specification.

use crate::conv::ConvChannel;
use crate::decoder::{Ctx, ModeParams, Protocol};
use crate::gen::{silence, FskModulator, Prepared, TxPulse};
use crate::items::*;
use crate::symbols::{Sym, SymAnswer, SyncAnswer, SyncPattern};
use crate::typed::DstarFields;
use sdrkit_algo_fec::conv::codes::DSTAR_K3;
use sdrkit_core::interface::Fields;
use sdrkit_core::media::Codec;
use sdrkit_core::publish::Published;
use sdrkit_core::publish::Declare;
use sdrkit_core::{Complex32, GenConfig, Value};
use sdrkit_proto_link::{aprs, dprs};
use sdrkit_util_bits::crc::catalog::CRC16_IBM_SDLC;
use sdrkit_util_bits::interleave::Permutation;
use sdrkit_util_bits::pack::{pack, unpack};
use sdrkit_util_bits::{seq, BitOrder, Pad};
use sdrkit_util_geo::nmea;
use std::sync::LazyLock;

/// The frame sync after the bit sync preamble.
pub const FRAME_SYNC: &str = "111011001010000";
/// The data sync in the slow data of every 21st voice frame (0x55 0x2D 0x16, least significant bit first).
pub const DATA_SYNC: &str = "101010101011010001101000";
/// The end of a transmission (0x55 x4, 0xC8, 0x7A, least significant bit first).
pub const END: &str = "101010101010101010101010101010100001001101011110";

/// The slow data scrambler (XORed with each frame's three bytes).
pub const SLOW_XOR: [u8; 3] = [0x70, 0x4F, 0x93];

/// The coded header bits.
pub const HEADER_BITS: usize = 660;

/// The header interleave: a 24-column matrix, 28 rows in the first 12 columns and 27 in the rest, filled by columns and sent by
/// rows (JARL D-STAR specification, radio header). The bit sent `i`-th is coded bit `header_source(i)`.
fn header_source(i: usize) -> usize {
    let rows = |c: usize| if c < 12 { 28 } else { 27 };
    let mut k = 0;
    for r in 0..28 {
        for c in 0..24 {
            if r < rows(c) {
                if k == i {
                    let start: usize = (0..c).map(rows).sum();
                    return start + r;
                }
                k += 1;
            }
        }
    }
    i
}

/// The header interleave as a permutation (coded bits to the order sent).
static HEADER_ORDER: LazyLock<Permutation> = LazyLock::new(|| {
    Permutation::gather_fn(HEADER_BITS, header_source).expect("the interleave is a permutation")
});

/// The header scrambler sequence (x^7 + x^4 + 1, register all ones), XORed onto the bits sent.
static HEADER_PN: LazyLock<[u8; HEADER_BITS]> = LazyLock::new(|| {
    let mut pn = [0u8; HEADER_BITS];
    let mut l = seq::DSTAR;
    l.fill_bits(&mut pn);
    pn
});

/// The header's convolutional decoder: 328 bits and 2 tail bits.
pub fn header_channel() -> ConvChannel {
    ConvChannel::new(DSTAR_K3, None, 330)
}

/// Bytes to bits, least significant bit first (D-STAR's order).
pub fn lsb_bits(bytes: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(bytes.len() * 8);
    unpack(bytes, BitOrder::Lsb, &mut out);
    out
}

/// Bits (least significant first) to bytes.
pub fn lsb_bytes(bits: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(bits.len().div_ceil(8));
    pack(bits, BitOrder::Lsb, Pad::Zero, &mut out);
    out
}

/// The radio header.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Header {
    pub flags: [u8; 3],
    pub rpt2: String,
    pub rpt1: String,
    pub ur: String,
    pub my: String,
    pub suffix: String,
}

impl Header {
    pub fn to_bytes(&self) -> [u8; 41] {
        let mut b = [b' '; 41];
        b[..3].copy_from_slice(&self.flags);
        let put = |b: &mut [u8; 41], at: usize, n: usize, s: &str| {
            for (i, c) in s.bytes().chain(std::iter::repeat(b' ')).take(n).enumerate() {
                b[at + i] = c;
            }
        };
        put(&mut b, 3, 8, &self.rpt2);
        put(&mut b, 11, 8, &self.rpt1);
        put(&mut b, 19, 8, &self.ur);
        put(&mut b, 27, 8, &self.my);
        put(&mut b, 35, 4, &self.suffix);
        let crc = CRC16_IBM_SDLC.checksum(&b[..39]);
        b[39..].copy_from_slice(&crc.to_le_bytes());
        b
    }
    pub fn from_bytes(b: &[u8]) -> Option<Header> {
        if b.len() < 41 || !CRC16_IBM_SDLC.verify(&b[..41], sdrkit_util_bits::crc::Trailer::Little)
        {
            return None;
        }
        let s = |a: usize, n: usize| String::from_utf8_lossy(&b[a..a + n]).to_string();
        Some(Header {
            flags: [b[0], b[1], b[2]],
            rpt2: s(3, 8),
            rpt1: s(11, 8),
            ur: s(19, 8),
            my: s(27, 8),
            suffix: s(35, 4),
        })
    }
    /// "via repeater", "simplex" and the other bits of flag 1.
    pub fn flag_text(&self) -> String {
        let f = self.flags[0];
        let mut v = vec![if f & 0x40 != 0 {
            "via repeater"
        } else {
            "simplex"
        }];
        if f & 0x80 != 0 {
            v.push("data");
        }
        if f & 0x20 != 0 {
            v.push("interruption");
        }
        if f & 0x08 != 0 {
            v.push("emergency");
        }
        v.join(", ")
    }
}

/// Encodes a header into the 660 bits sent after the frame sync: the 328 header bits and 2 zero tail bits, the rate 1/2 code,
/// the interleave and the scrambler.
pub fn encode_header(h: &Header) -> Vec<u8> {
    let mut bits = lsb_bits(&h.to_bytes());
    bits.extend([0, 0]);
    let mut coded = Vec::with_capacity(HEADER_BITS);
    header_channel().encode(&bits, &mut coded);
    let mut out = vec![0u8; HEADER_BITS];
    if HEADER_ORDER.apply(&coded, &mut out).is_err() {
        out.clear();
    }
    for (o, p) in out.iter_mut().zip(HEADER_PN.iter()) {
        *o ^= p;
    }
    out
}

/// Decodes the 660 header bits (soft values, positive means 1). The frame is terminated by its 2 zero tail bits. Returns the
/// header when its CRC checks, and the bits repaired.
pub fn decode_header(conv: &mut ConvChannel, soft: &[f32]) -> (Option<Header>, usize) {
    if soft.len() < HEADER_BITS {
        return (None, 0);
    }
    let mut sent = [0f32; HEADER_BITS];
    for (i, (v, p)) in soft.iter().zip(HEADER_PN.iter()).enumerate() {
        sent[i] = if *p != 0 { -v } else { *v };
    }
    let mut coded = [0f32; HEADER_BITS];
    if HEADER_ORDER.invert(&sent, &mut coded).is_err() {
        return (None, 0);
    }
    match conv.decode(&coded, 330, true) {
        Ok((bits, errs)) => (Header::from_bytes(&lsb_bytes(&bits[..328])), errs),
        Err(_) => (None, 0),
    }
}

#[derive(Clone, Debug, Default)]
struct Slow {
    half: Option<[u8; 3]>,
    message: [Option<[u8; 5]>; 4],
    gps: String,
    header: Vec<u8>,
    squelch: Option<u8>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum State {
    Idle,
    Header,
    Voice,
}

/// The D-STAR protocol.
pub struct DStar {
    state: State,
    bits: Vec<f32>,
    frame: usize,
    slow: Slow,
    headers_ok: u64,
    voice_frames: u64,
    lost: usize,
    out_of_sync: bool,
    bad_run: u8,
    inverted: bool,
    texts_sent: Vec<String>,
    idle: usize,
    held: Option<(Vec<f32>, usize)>,
    sync_inverted: bool,
    sync_at: f64,
    conv: ConvChannel,
}

impl Default for DStar {
    fn default() -> Self {
        DStar {
            state: State::Idle,
            bits: Vec::with_capacity(700),
            frame: 0,
            slow: Slow::default(),
            headers_ok: 0,
            voice_frames: 0,
            lost: 0,
            out_of_sync: false,
            bad_run: 0,
            inverted: false,
            texts_sent: Vec::new(),
            idle: 0,
            held: None,
            sync_inverted: false,
            sync_at: 0.0,
            conv: header_channel(),
        }
    }
}

const KEY: u32 = 1;

/// The frames in a row that fail their Golay codes that put the voice out of sync, as a data sync missed on its schedule
/// does. A frame fails when the speech decoder would repeat it ([`sdrkit_codec_mbe::DstarErrors::repeats`]) or when the
/// codes correct more than [`BAD_ERRORS`] bits; noise fails about 39 frames in 40.
const BAD_RUN: u8 = 2;
/// The most bits that the Golay codes of a frame may correct for the frame not to fail ([`BAD_RUN`]).
const BAD_ERRORS: u32 = 4;
/// The most bits that the Golay codes of a frame may correct while the voice is out of sync for the frame to count as
/// received; noise passes this test about once in 7000 frames. A frame that fails it is a lost frame.
const OUT_OF_SYNC_ERRORS: u32 = 2;

impl DStar {
    fn start(&mut self, h: &Header, from_slow: bool, ctx: &mut Ctx) {
        if let Some(c) = ctx.call(KEY) {
            if c.src == Ident::Text(h.my.trim_end().to_string()) {
                return;
            }
        }
        let mut rec = CallRecord {
            kind: CallKind::Voice,
            src: Ident::Text(h.my.trim_end().to_string()),
            dst: Ident::Text(h.ur.trim_end().to_string()),
            group: h.ur.trim_end() == "CQCQCQ",
            emergency: h.flags[0] & 0x08 != 0,
            encryption: Encryption::clear(),
            ..Default::default()
        };
        rec.set(CallDetail::My, Value::Text(h.my.trim_end().into()));
        rec.set(CallDetail::Suffix, Value::Text(h.suffix.trim_end().into()));
        rec.set(CallDetail::Ur, Value::Text(h.ur.trim_end().into()));
        rec.set(CallDetail::Rpt1, Value::Text(h.rpt1.trim_end().into()));
        rec.set(CallDetail::Rpt2, Value::Text(h.rpt2.trim_end().into()));
        rec.set(CallDetail::Flag1, Value::I64(i64::from(h.flags[0])));
        rec.set(CallDetail::Flags, Value::Text(h.flag_text()));
        rec.set(CallDetail::HeaderOk, Value::Bool(true));
        rec.set(CallDetail::HeaderFromSlowData, Value::Bool(from_slow));
        ctx.start_call(KEY, rec, from_slow);
        ctx.set_encryption(KEY, Encryption::clear());
        self.slow = Slow::default();
        self.texts_sent.clear();
    }

    /// One voice frame of 96 soft bits: its speech goes to the call, its slow data to [`DStar::slow_block`]. While the
    /// voice is out of sync (a data sync missed, or [`BAD_RUN`] frames in a row that fail their Golay codes) a frame with
    /// more than [`OUT_OF_SYNC_ERRORS`] corrected bits is a lost frame, which the speech decoder conceals; its slow data is
    /// still read. A data sync or a header puts the voice back in sync.
    fn voice_frame(&mut self, b: &[f32], ctx: &mut Ctx) {
        let mut hard = [0u8; 96];
        for (h, &v) in hard.iter_mut().zip(b) {
            *h = (v > 0.0) as u8;
        }
        self.voice_frames += 1;
        let pos = self.frame % 21;
        self.frame += 1;
        let e = sdrkit_codec_mbe::dstar_channel_errors(&hard[..72]);
        let bad = e.repeats() || e.corrected > BAD_ERRORS;
        self.bad_run = if bad { self.bad_run.saturating_add(1) } else { 0 };
        if self.bad_run >= BAD_RUN {
            self.out_of_sync = true;
        }
        let lost = self.out_of_sync && (e.repeats() || e.corrected > OUT_OF_SYNC_ERRORS);
        if ctx.has_call(KEY) {
            ctx.update_call(KEY, |c| c.voice_frames += 1);
            let spec = crate::voice::FrameSpec {
                codec: Codec::Ambe,
                packaging: crate::voice::DSTAR_PACKAGING,
                codec_mode: None,
                duration_ms: 20,
                stack: "dstar",
                carrier_hz: ctx.carrier_hz,
                slot: 0,
            };
            if lost {
                ctx.voice(KEY, crate::voice::erasure(spec, ctx.now));
            } else {
                let cw = crate::voice::channel_bits::<9>(&hard[..72]);
                ctx.speech(KEY, spec, &cw, ctx.now, false, None);
            }
        }
        if pos == 0 {
            // The data sync frame.
            self.slow.half = None;
            return;
        }
        let mut three = [0u8; 3];
        for (i, (x, k)) in three.iter_mut().zip(SLOW_XOR).enumerate() {
            let bits = &hard[72 + 8 * i..80 + 8 * i];
            *x = bits.iter().rev().fold(0, |a, &bit| a << 1 | bit) ^ k;
        }
        match self.slow.half.take() {
            None => self.slow.half = Some(three),
            Some(first) => {
                let block = [first[0], first[1], first[2], three[0], three[1], three[2]];
                self.slow_block(&block, ctx);
            }
        }
    }

    fn slow_block(&mut self, b: &[u8; 6], ctx: &mut Ctx) {
        let kind = b[0] >> 4;
        let n = (b[0] & 0x0F) as usize;
        match kind {
            0x3 => {
                // GPS data: NMEA sentences and D-PRS, as text.
                for &c in &b[1..1 + n.min(5)] {
                    if c == b'\n' || c == b'\r' {
                        let line = std::mem::take(&mut self.slow.gps);
                        self.gps_line(line.trim(), ctx);
                    } else if self.slow.gps.len() < 256 {
                        self.slow.gps.push(c as char);
                    }
                }
            }
            0x4 => {
                if n < 4 {
                    let mut five = [0u8; 5];
                    five.copy_from_slice(&b[1..6]);
                    self.slow.message[n] = Some(five);
                    if self.slow.message.iter().all(|m| m.is_some()) {
                        let s: String = self
                            .slow
                            .message
                            .iter()
                            .flatten()
                            .flatten()
                            .map(|&c| c as char)
                            .collect();
                        let text = s.trim_end().to_string();
                        if !self.texts_sent.contains(&text) {
                            self.texts_sent.push(text.clone());
                            ctx.update_call(KEY, |c| c.text = text.clone());
                            let src = ctx.call(KEY).map(|c| c.src).unwrap_or_default();
                            ctx.message(MessageRecord {
                                src,
                                format: MessageFormat::SlowData,
                                data_format: None,
                                text,
                                crc_ok: true,
                                time: ctx.now,
                                ..Default::default()
                            });
                        }
                    }
                }
            }
            0x5 => {
                self.slow.header.extend_from_slice(&b[1..1 + n.min(5)]);
                if self.slow.header.len() >= 41 {
                    let hb: Vec<u8> = self.slow.header.drain(..41).collect();
                    if let Some(h) = Header::from_bytes(&hb) {
                        if !ctx.has_call(KEY) {
                            self.headers_ok += 1;
                            self.start(&h, true, ctx);
                        }
                    }
                    self.slow.header.clear();
                }
            }
            0xC => {
                // Code squelch: the code in the next byte.
                let code = b[1];
                if self.slow.squelch != Some(code) {
                    self.slow.squelch = Some(code);
                    ctx.update_call(KEY, |c| {
                        c.set(CallDetail::Squelch, Value::I64(i64::from(code)))
                    });
                }
            }
            _ => {}
        }
    }

    /// One line of GPS text: a D-PRS position (its CRC must check), or an NMEA RMC or GGA sentence (its `*hh` checksum must
    /// check).
    fn gps_line(&mut self, line: &str, ctx: &mut Ctx) {
        let src = ctx.call(KEY).map(|c| c.src).unwrap_or_default();
        if line.starts_with("$$CRC") {
            if let Some(p) = parse_dprs(line) {
                ctx.update_call(KEY, |c| {
                    c.set(
                        CallDetail::Position,
                        Value::Position {
                            lat: p.lat,
                            lon: p.lon,
                        },
                    )
                });
                ctx.position(PositionRecord { time: ctx.now, ..p });
            }
        } else if let Some(mut p) = parse_nmea(line) {
            p.src = src;
            p.time = ctx.now;
            if p.alt_m.is_none() {
                // An altitude from a GGA of the same position.
                p.alt_m = ctx.with_tracker(|t| {
                    t.lists
                        .positions
                        .iter()
                        .next_back()
                        .filter(|q| q.source == PositionFormat::Nmea)
                        .and_then(|q| q.alt_m)
                });
            }
            self.merge_nmea(p, ctx);
        }
    }

    /// RMC and GGA of the same fix make one position.
    fn merge_nmea(&mut self, p: PositionRecord, ctx: &mut Ctx) {
        let merged = ctx.with_tracker(|t| {
            if let Some(q) = t.lists.positions.newest_mut() {
                if q.source == PositionFormat::Nmea
                    && q.src == p.src
                    && (q.lat - p.lat).abs() < 1e-5
                    && (q.lon - p.lon).abs() < 1e-5
                    && p.time - q.time < 2.0
                {
                    q.alt_m = q.alt_m.or(p.alt_m);
                    q.speed_kmh = q.speed_kmh.or(p.speed_kmh);
                    q.course_deg = q.course_deg.or(p.course_deg);
                    t.dirty = true;
                    return true;
                }
            }
            false
        });
        ctx.update_call(KEY, |c| {
            c.set(
                CallDetail::Position,
                Value::Position {
                    lat: p.lat,
                    lon: p.lon,
                },
            )
        });
        if !merged {
            ctx.position(p);
        }
    }
}

/// An NMEA RMC sentence (position, speed in km/h, course) or GGA sentence (position, altitude above mean sea level) with a
/// correct checksum (NMEA 0183). Other sentences, and RMC without a valid fix, give `None`.
pub fn parse_nmea(line: &str) -> Option<PositionRecord> {
    let s = nmea::parse(line).ok()?;
    match s.formatter() {
        "RMC" => {
            let r = nmea::Rmc::from_sentence(&s).ok()?;
            let p = r.position.filter(|_| r.valid)?;
            Some(PositionRecord {
                lat: p.lat_deg,
                lon: p.lon_deg,
                speed_kmh: r.speed_kn.map(|k| k * 1.852),
                course_deg: r.course_deg,
                source: PositionFormat::Nmea,
                ..Default::default()
            })
        }
        "GGA" => {
            let g = nmea::Gga::from_sentence(&s).ok()?;
            let p = g.position?;
            Some(PositionRecord {
                lat: p.lat_deg,
                lon: p.lon_deg,
                alt_m: g.altitude_msl_m,
                source: PositionFormat::Nmea,
                ..Default::default()
            })
        }
        _ => None,
    }
}

/// A D-PRS line `$$CRCxxxx,CALL>DEST,PATH:info`: the APRS position of the TNC2 packet, when the line's CRC checks (D-PRS, APRS
/// 1.0.1 chapter 8).
pub fn parse_dprs(line: &str) -> Option<PositionRecord> {
    let d = dprs::parse(line).ok().filter(|d| d.crc_ok)?;
    let call = d.tnc2.split('>').next()?.to_string();
    let pkt = aprs::parse_tnc2(d.tnc2).ok()?;
    let aprs::Body::Position { report, .. } = pkt.body else {
        return None;
    };
    Some(PositionRecord {
        src: Ident::Text(call),
        lat: report.position.lat_deg,
        lon: report.position.lon_deg,
        alt_m: report.altitude_m,
        speed_kmh: report.speed_kn.map(|k| k * 1.852),
        course_deg: report.course_deg.filter(|&c| c > 0).map(f64::from),
        source: PositionFormat::Dprs,
        comment: report.comment.trim().to_string(),
        ..Default::default()
    })
}

impl Protocol for DStar {
    fn params(&self) -> ModeParams {
        ModeParams {
            mode: "dstar",
            name: "D-STAR",
            baud: 4800.0,
            two_level: true,
            channel_hz: 6_250.0,
            calls: &crate::results::CALLS_300,
            message_cap: 100,
            polarity_rule: false,
            clock_loop: true,
            extra_baud: None,
            stream_rate: crate::front::RATE,
        }
    }

    fn patterns(&self) -> Vec<SyncPattern> {
        vec![
            SyncPattern::from_bits(0, FRAME_SYNC, 8).with_max_err(1),
            SyncPattern::from_bits(1, DATA_SYNC, 72).with_max_err(2),
            SyncPattern::from_bits(2, END, 0).with_max_err(3),
        ]
    }

    fn on_sync(
        &mut self,
        id: u8,
        inverted: bool,
        pre: &[Sym],
        _at: u64,
        _tracking: bool,
        ctx: &mut Ctx,
    ) -> SyncAnswer {
        // Within a transmission the polarity is known: a sync of the other polarity is a false one.
        if self.state != State::Idle && inverted != self.inverted {
            return SyncAnswer::Ignore;
        }
        self.sync_inverted = inverted;
        match id {
            // Inside a transmission whose data syncs arrive, a frame sync is a false one.
            0 if self.state == State::Voice && self.lost == 0 => return SyncAnswer::Ignore,
            0 if self.state == State::Header && self.bits.len() > 40 => return SyncAnswer::Ignore,
            0 => {
                // The frame sync is preceded by the bit sync (1010...): check its last eight bits.
                let ok = pre[..8]
                    .iter()
                    .enumerate()
                    .filter(|(i, s)| (s.d > 0) == (i % 2 == 0))
                    .count()
                    >= 6;
                if !ok {
                    return SyncAnswer::Ignore;
                }
                self.state = State::Header;
                self.bits.clear();
                self.idle = 0;
                self.sync_at = ctx.now;
            }
            1 => {
                if self.state == State::Header {
                    return SyncAnswer::Ignore;
                }
                // Realign: the voice of this frame is in `pre` (a frame held for it is replaced).
                self.held = None;
                self.out_of_sync = false;
                self.bad_run = 0;
                self.inverted = self.sync_inverted;
                self.frame = 0;
                self.bits.clear();
                let voice: Vec<f32> = pre[..72].iter().map(|s| s.v).collect();
                let mut b = voice;
                b.extend(pre[72..].iter().map(|s| s.v));
                if self.state == State::Voice || ctx.has_call(KEY) {
                    self.voice_frame(&b, ctx);
                } else {
                    // A transmission joined late: frames count, the header can come in slow data.
                    self.state = State::Voice;
                    self.voice_frame(&b, ctx);
                }
                self.state = State::Voice;
                self.lost = 0;
                // The data sync on its schedule confirms the voice frames: they carry no error check of their own.
                ctx.confirmed();
            }
            _ => {
                if ctx.has_call(KEY) {
                    ctx.end_call(KEY, true);
                }
                self.state = State::Idle;
            }
        }
        SyncAnswer::Take
    }

    fn on_symbol(&mut self, s: Sym, _at: u64, ctx: &mut Ctx) -> SymAnswer {
        match self.state {
            State::Idle => {
                self.idle += 1;
                if self.idle > 2000 {
                    self.idle = 0;
                    return SymAnswer::Stop;
                }
            }
            State::Header => {
                self.bits.push(s.v.clamp(-1.5, 1.5));
                if self.bits.len() == 660 {
                    let (h, errs) = decode_header(&mut self.conv, &self.bits);
                    ctx.identity(h.is_some());
                    ctx.block(h.is_some(), 660, errs);
                    if let Some(h) = h {
                        self.inverted = self.sync_inverted;
                        self.headers_ok += 1;
                        // The call starts at its frame sync.
                        let now = ctx.now;
                        ctx.now = self.sync_at;
                        self.start(&h, false, ctx);
                        ctx.now = now;
                        self.state = State::Voice;
                        self.frame = 0;
                        self.lost = 0;
                        self.out_of_sync = false;
                        self.bad_run = 0;
                    } else {
                        self.state = State::Idle;
                    }
                    self.bits.clear();
                }
            }
            State::Voice => {
                self.bits.push(s.v.clamp(-1.5, 1.5));
                // The frame that carries the data sync waits a few symbols for the sync event.
                let overdue = self.held.take_if(|h| {
                    h.1 += 1;
                    h.1 > 6
                });
                if let Some((b, _)) = overdue {
                    self.lost += 1;
                    self.out_of_sync = true;
                    if self.lost > 3 {
                        if ctx.has_call(KEY) {
                            ctx.end_call(KEY, false);
                        }
                        self.state = State::Idle;
                        self.bits.clear();
                        return SymAnswer::Stop;
                    }
                    self.voice_frame(&b, ctx);
                }
                if self.bits.len() == 96 && self.frame.is_multiple_of(21) && self.held.is_none() {
                    self.held = Some((std::mem::take(&mut self.bits), 0));
                    return SymAnswer::Continue;
                }
                if self.bits.len() == 96 {
                    let b = std::mem::take(&mut self.bits);
                    if crate::symbols::carrier_gone_levels(b.iter().copied()) {
                        if ctx.has_call(KEY) {
                            ctx.end_call(KEY, false);
                        }
                        self.state = State::Idle;
                        return SymAnswer::Stop;
                    }
                    self.voice_frame(&b, ctx);
                    self.bits = b;
                    self.bits.clear();
                }
            }
        }
        SymAnswer::Continue
    }

    fn fields(&self) -> Vec<&'static dyn Declare> {
        <DstarFields as Fields>::SHAPE.fields.to_vec()
    }

    fn publish(&self, r: &mut Published) {
        r.put(&DstarFields {
            headers_ok: self.headers_ok as i64,
            dstar_voice_frames: self.voice_frames as i64,
            sync_inverted: self.inverted,
        });
    }

    fn codec(&self) -> Codec {
        Codec::Ambe
    }
}

// ---------------------------------------------------------------------------------------------------------------------------
// Generator.

/// One D-STAR transmission of the test schedule.
pub struct Tx {
    pub header: Header,
    pub message: String,
    pub gps: Vec<String>,
    pub squelch: Option<u8>,
    pub frames: usize,
}

/// The start of a transmission: the bit sync (64 bits `1010…`), the frame sync and the coded header.
pub fn start_bits(h: &Header) -> Vec<u8> {
    let mut bits: Vec<u8> = (0..64).map(|i| ((i + 1) % 2) as u8).collect();
    bits.extend(FRAME_SYNC.bytes().map(|c| c - b'0'));
    bits.extend(encode_header(h));
    bits
}

/// The end of a transmission: the end pattern.
pub fn end_bits() -> Vec<u8> {
    END.bytes().map(|c| c - b'0').collect()
}

/// The slow-data blocks of a transmission in the order they are sent: the message `message` (4 blocks of 5 characters,
/// padded with spaces), the header copy (9 blocks), the GPS text `gps` (lines each ended by a carriage return) and the code
/// squelch `squelch`. Unused bytes are the filler 0x66.
pub fn slow_blocks(h: &Header, message: &str, gps: &[String], squelch: Option<u8>) -> Vec<[u8; 6]> {
    let mut blocks: Vec<[u8; 6]> = Vec::new();
    let msg: Vec<u8> = message
        .bytes()
        .chain(std::iter::repeat(b' '))
        .take(20)
        .collect();
    for k in 0..4 {
        let mut b = [0x40 | k as u8, 0, 0, 0, 0, 0];
        b[1..].copy_from_slice(&msg[k * 5..k * 5 + 5]);
        blocks.push(b);
    }
    let hb = h.to_bytes();
    for c in hb.chunks(5) {
        let mut b = [0x50 | c.len() as u8, 0x66, 0x66, 0x66, 0x66, 0x66];
        b[1..1 + c.len()].copy_from_slice(c);
        blocks.push(b);
    }
    let mut gps_text = String::new();
    for g in gps {
        gps_text.push_str(g);
        gps_text.push('\r');
    }
    for c in gps_text.as_bytes().chunks(5) {
        let mut b = [0x30 | c.len() as u8, 0x66, 0x66, 0x66, 0x66, 0x66];
        b[1..1 + c.len()].copy_from_slice(c);
        blocks.push(b);
    }
    if let Some(q) = squelch {
        blocks.push([0xC2, q, q, 0x66, 0x66, 0x66]);
    }
    blocks
}

/// The voice frames of a transmission: 72 voice bits and 24 slow-data bits each. Frame 0 of every 21 carries the data
/// sync; the others carry the slow-data blocks in turn, 3 bytes a frame, XORed with [`SLOW_XOR`].
#[derive(Clone, Debug)]
pub struct VoiceFrames {
    blocks: Vec<[u8; 6]>,
    next: usize,
    half: Option<[u8; 3]>,
    frame: usize,
}

impl VoiceFrames {
    /// The frames that carry `blocks` (at least one block).
    pub fn new(blocks: Vec<[u8; 6]>) -> VoiceFrames {
        VoiceFrames {
            blocks,
            next: 0,
            half: None,
            frame: 0,
        }
    }

    /// The voice frames made so far.
    pub fn frames(&self) -> usize {
        self.frame
    }

    /// The frames that send every slow-data block once: whole groups of 21 frames.
    pub fn cycle(&self) -> usize {
        21 * (2 * self.blocks.len()).div_ceil(20)
    }

    /// Appends the 96 bits of the next frame with the 72 voice bits `voice` to `out`.
    pub fn push(&mut self, voice: &[u8], out: &mut Vec<u8>) {
        out.extend_from_slice(&voice[..72]);
        let f = self.frame;
        self.frame += 1;
        if f.is_multiple_of(21) {
            out.extend(DATA_SYNC.bytes().map(|c| c - b'0'));
            self.half = None;
            return;
        }
        let three = match self.half.take() {
            Some(h) => h,
            None => {
                let b = self.blocks[self.next % self.blocks.len()];
                self.next += 1;
                self.half = Some([b[3], b[4], b[5]]);
                [b[0], b[1], b[2]]
            }
        };
        for (x, k) in three.iter().zip(SLOW_XOR) {
            let s = x ^ k;
            out.extend((0..8).map(|i| s >> i & 1));
        }
    }
}

/// The bits of the transmission `t`. Its voice frames carry the test speech ([`crate::voice::test_speech_dstar`]) in
/// turn.
fn tx_bits(t: &Tx) -> Vec<u8> {
    let mut bits = start_bits(&t.header);
    let mut frames = VoiceFrames::new(slow_blocks(&t.header, &t.message, &t.gps, t.squelch));
    let speech = crate::voice::test_speech_dstar();
    for k in 0..t.frames {
        frames.push(&speech[k % speech.len()], &mut bits);
    }
    bits.extend(end_bits());
    bits
}

fn nmea_with_checksum(body: &str) -> String {
    format!("${body}*{:02X}", nmea::checksum(body.as_bytes()))
}

fn dprs_line(tnc2: &str) -> String {
    let mut line = String::new();
    dprs::encode(tnc2, &mut line);
    line.trim_end_matches('\r').to_string()
}

/// The D-STAR test signal: a call via a repeater with a message and NMEA positions, and a simplex call with a D-PRS position and
/// a code squelch; the voice frames of both carry the test speech.
pub fn render() -> Vec<Complex32> {
    let mut m = FskModulator::new(4800.0, TxPulse::Gaussian(0.5), 1200.0);
    let mut out = Vec::new();
    silence(0.3, &mut out);
    let calls = [
        Tx {
            header: Header {
                flags: [0x40, 0, 0],
                rpt2: "N0RPT  G".into(),
                rpt1: "N0RPT  B".into(),
                ur: "CQCQCQ".into(),
                my: "N0CALL".into(),
                suffix: "ID51".into(),
            },
            message: "TEST SIGNAL ONE 73".into(),
            gps: vec![
                nmea_with_checksum("GPRMC,120000,A,5441.2320,N,02516.7820,E,19.438,90.0,010125,,"),
                nmea_with_checksum(
                    "GPGGA,120000,5441.2320,N,02516.7820,E,1,08,1.0,112.0,M,0.0,M,,",
                ),
            ],
            squelch: None,
            frames: 200,
        },
        Tx {
            header: Header {
                flags: [0, 0, 0],
                rpt2: "DIRECT".into(),
                rpt1: "DIRECT".into(),
                ur: "CQCQCQ".into(),
                my: "N1TEST".into(),
                suffix: String::new(),
            },
            message: "Hello via simplex".into(),
            gps: vec![dprs_line(
                "N0CALL-7>API51,DSTAR*:!5453.91N/02354.22E>test position",
            )],
            squelch: Some(12),
            frames: 150,
        },
    ];
    for c in &calls {
        let bits = tx_bits(c);
        let lv: Vec<f32> = bits
            .iter()
            .map(|&b| if b != 0 { 1.0 } else { -1.0 })
            .collect();
        m.symbols(&lv, &mut out);
        m.flush(8, &mut out);
        m.reset();
        silence(0.6, &mut out);
    }
    out
}

/// The D-STAR generator from the settings of the stack.
pub fn generator(cfg: &GenConfig) -> Prepared {
    Prepared::new(render(), cfg.rate_hz, true).with_info(6_250.0, 4800.0, 4800.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_round_trips() {
        let h = Header {
            flags: [0x40, 0, 0],
            rpt2: "N0RPT  G".into(),
            rpt1: "N0RPT  B".into(),
            ur: "CQCQCQ  ".into(),
            my: "N0CALL  ".into(),
            suffix: "ID51".into(),
        };
        let mut bits: Vec<f32> = encode_header(&h)
            .iter()
            .map(|&b| if b != 0 { 1.0 } else { -1.0 })
            .collect();
        bits[100] = -bits[100];
        bits[400] = -bits[400];
        let (got, errs) = decode_header(&mut header_channel(), &bits);
        assert_eq!(got.unwrap(), h);
        assert_eq!(errs, 2);
    }

    #[test]
    fn positions_parse() {
        let p = parse_nmea(&nmea_with_checksum(
            "GPRMC,120000,A,5441.2320,N,02516.7820,E,19.438,90.0,010125,,",
        ))
        .unwrap();
        assert!((p.lat - 54.6872).abs() < 1e-6 && (p.lon - 25.2797).abs() < 1e-6);
        assert!((p.speed_kmh.unwrap() - 36.0).abs() < 0.01);
        let g = parse_nmea(&nmea_with_checksum(
            "GPGGA,120000,5441.2320,N,02516.7820,E,1,08,1.0,112.0,M,0.0,M,,",
        ))
        .unwrap();
        assert_eq!(g.alt_m, Some(112.0));
        let d = parse_dprs(&dprs_line(
            "N0CALL-7>API51,DSTAR*:!5453.91N/02354.22E>test position",
        ))
        .unwrap();
        assert_eq!(d.src, Ident::Text("N0CALL-7".into()));
        assert!((d.lat - 54.8985).abs() < 1e-4 && (d.lon - 23.9037).abs() < 2e-4);
        assert_eq!(d.comment, "test position");
    }

    #[test]
    fn a_header_with_errors_on_its_last_bit_decodes_as_a_terminated_frame() {
        let h = Header {
            flags: [0x40, 0, 0],
            rpt2: "N0RPT  G".into(),
            rpt1: "N0RPT  B".into(),
            ur: "CQCQCQ  ".into(),
            my: "N0CALL  ".into(),
            suffix: "ID51".into(),
        };
        let mut soft: Vec<f32> = encode_header(&h)
            .iter()
            .map(|&b| if b != 0 { 1.0 } else { -1.0 })
            .collect();
        // Both coded bits of the last header bit (coded bits 654 and 655) are wrong.
        for c in [654, 655] {
            let at = HEADER_ORDER.dest(c).unwrap();
            soft[at] = -soft[at];
        }
        assert_eq!(decode_header(&mut header_channel(), &soft).0, Some(h));
        // Read as an open frame, which ignores the 2 zero tail bits, the same bits do not decode.
        let mut sent = [0f32; HEADER_BITS];
        for (i, (v, p)) in soft.iter().zip(HEADER_PN.iter()).enumerate() {
            sent[i] = if *p != 0 { -v } else { *v };
        }
        let mut coded = [0f32; HEADER_BITS];
        HEADER_ORDER.invert(&sent, &mut coded).unwrap();
        let mut open = header_channel();
        let (bits, _) = open.decode(&coded, 330, false).unwrap();
        assert_eq!(Header::from_bytes(&lsb_bytes(&bits[..328])), None);
    }

    #[test]
    fn gps_text_with_a_wrong_checksum_or_crc_has_no_position() {
        let rmc =
            nmea_with_checksum("GPRMC,120000,A,5441.2320,N,02516.7820,E,19.438,90.0,010125,,");
        assert!(parse_nmea(&rmc).is_some());
        let bad = format!("{}00", &rmc[..rmc.len() - 2]);
        assert_ne!(bad, rmc);
        assert!(parse_nmea(&bad).is_none());
        let good = dprs_line("N0CALL-7>API51,DSTAR*:!5453.91N/02354.22E>test position");
        assert!(parse_dprs(&good).is_some());
        let bad = good.replacen(&good[5..9], "0000", 1);
        assert!(parse_dprs(&bad).is_none());
    }
}
