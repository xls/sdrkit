//! Yaesu System Fusion: the frame information channel (FICH), the data channel (DCH) of every frame kind with its
//! whitening, the call signs of header and terminator frames, the text and GPS of the data channel, and the speech of
//! V/D mode 1, V/D mode 2 and voice full rate.
//! Standard: Yaesu Amateur Radio Digital Standards, revision 1.02.

use crate::bits::{bits_to_bytes, bytes_to_bits};
use crate::conv::ConvChannel;
use crate::decoder::{Ctx, ModeParams, Protocol};
use crate::gen::{bits_to_levels, silence, FskModulator, Prepared, TxPulse};
use crate::items::*;
use crate::symbols::{Sym, SymAnswer, SyncAnswer, SyncPattern};
use crate::typed::YsfFields;
use sdrkit_algo_fec::conv::codes::NXDN_K5;
use sdrkit_algo_fec::small::SmallCode;
use sdrkit_core::interface::Fields;
use sdrkit_core::media::Codec;
use sdrkit_core::publish::{field, Declare, Published};
use sdrkit_core::{Complex32, GenConfig, Setting, Value};
use sdrkit_graph::SpeechPackaging;
use sdrkit_util_bits::crc::catalog::CRC16_GSM;
use sdrkit_util_bits::interleave::Permutation;
use sdrkit_util_bits::lfsr::{Additive, Lfsr};
use sdrkit_util_bits::pack::{push_u64, to_u64};
use std::ops::Range;
use std::sync::LazyLock;

pub const FSW: &str = "D471C9634D";

/// The frame information channel: 32 bits, the first one bit 31.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Fich {
    /// Frame information: 0 header, 1 communication, 2 terminator, 3 test.
    pub fi: u8,
    /// Call sign information.
    pub cs: u8,
    /// Call mode: 0 group CQ, 1 radio ID, 3 individual.
    pub cm: u8,
    /// Block number.
    pub bn: u8,
    /// Block total.
    pub bt: u8,
    /// Frame number.
    pub fn_: u8,
    /// Frame total.
    pub ft: u8,
    /// Deviation: false wide, true narrow.
    pub dev: bool,
    /// Message routing.
    pub mr: u8,
    /// The path goes through the Internet.
    pub voip: bool,
    /// Data type: 0 V/D mode 1, 1 data FR, 2 V/D mode 2, 3 voice FR.
    pub dt: u8,
    /// Squelch type: true when the squelch code is off.
    pub sql: bool,
    /// Squelch code, 7 bits.
    pub sc: u8,
}

impl Fich {
    /// The 32 bits, bit 31 first: FI, CS, CM, BN, BT, FN, FT, one reserved bit, Dev, MR, VoIP, DT, SQL, SC.
    pub fn to_bits(&self) -> Vec<u8> {
        let mut b = Vec::with_capacity(48);
        for (v, n) in [
            (self.fi, 2),
            (self.cs, 2),
            (self.cm, 2),
            (self.bn, 2),
            (self.bt, 2),
            (self.fn_, 3),
            (self.ft, 3),
            (0, 1),
            (self.dev as u8, 1),
            (self.mr, 3),
            (self.voip as u8, 1),
            (self.dt, 2),
            (self.sql as u8, 1),
            (self.sc, 7),
        ] {
            push_u64(&mut b, u64::from(v) & ((1 << n) - 1), n);
        }
        b
    }

    /// The fields of the first 32 bits of `b`.
    pub fn from_bits(b: &[u8]) -> Fich {
        let f = |a: usize, n: usize| to_u64(&b[a..a + n]) as u8;
        Fich {
            fi: f(0, 2),
            cs: f(2, 2),
            cm: f(4, 2),
            bn: f(6, 2),
            bt: f(8, 2),
            fn_: f(10, 3),
            ft: f(13, 3),
            dev: b[17] != 0,
            mr: f(18, 3),
            voip: b[21] != 0,
            dt: f(22, 2),
            sql: b[24] != 0,
            sc: f(25, 7),
        }
    }
}

/// Extended Golay (24,12) of the FICH words.
static FICH_WORD: LazyLock<SmallCode> = LazyLock::new(SmallCode::golay_24_12);

/// The dibit interleave of `20 m` dibits: coded dibit d is sent as dibit 20 (d mod m) + floor(d / m).
fn dibit_interleave(m: usize) -> Permutation {
    Permutation::from_fn(40 * m, |i| {
        let d = i / 2;
        2 * (20 * (d % m) + d / m) + i % 2
    })
    .expect("a permutation")
}

/// The interleave of the FICH and of the 200-bit DCH unit of V/D mode 2 (5 by 20 dibits).
static SHORT: LazyLock<Permutation> = LazyLock::new(|| dibit_interleave(5));
/// The interleave of the 360-bit DCH unit (9 by 20 dibits).
static LONG: LazyLock<Permutation> = LazyLock::new(|| dibit_interleave(9));

/// The convolutional channel of the FICH and the DCH (up to 180 input bits with the tail).
pub fn channel() -> ConvChannel {
    ConvChannel::new(NXDN_K5, None, 180)
}

/// The interleave for `n` coded bits (200 or 360).
fn interleaver(n: usize) -> Option<&'static Permutation> {
    match n {
        200 => Some(&SHORT),
        360 => Some(&LONG),
        _ => None,
    }
}

/// Interleaves 200 or 360 coded bits; empty for another length.
fn interleave(c: &[u8]) -> Vec<u8> {
    let mut out = vec![0u8; c.len()];
    if interleaver(c.len()).is_none_or(|p| p.apply(c, &mut out).is_err()) {
        out.clear();
    }
    out
}

/// De-interleaves 200 or 360 soft values.
fn deinterleave(s: &[f32]) -> Option<Vec<f32>> {
    let mut out = vec![0f32; s.len()];
    interleaver(s.len())?.invert(s, &mut out).ok()?;
    Some(out)
}

/// The whitening sequence PN(9,5): s[n + 9] = s[n + 4] XOR s[n], s[0..9] = 1 0 0 1 0 0 1 1 1. It restarts for every
/// unit.
fn whitening() -> Additive {
    Additive::new(Lfsr::recurrence(9, 0x11, 0x1C9))
}

/// FICH: 32 bits + CRC-16, four Golay (24,12) words, 4 tail bits, rate 1/2, the 5 by 20 dibit interleave: 200 bits.
/// No whitening.
pub fn encode_fich(f: &Fich) -> Vec<u8> {
    let mut b = f.to_bits();
    let crc = CRC16_GSM.bits(&b);
    push_u64(&mut b, crc as u64, 16);
    let mut g = Vec::with_capacity(100);
    for k in 0..4 {
        let w = FICH_WORD.encode(to_u64(&b[k * 12..k * 12 + 12]) as u128);
        push_u64(&mut g, w as u64, 24);
    }
    g.extend([0, 0, 0, 0]);
    let mut c = Vec::with_capacity(200);
    channel().encode(&g, &mut c);
    interleave(&c)
}

/// Decodes the 200 soft values of a FICH: the FICH and the bits repaired, when every word decodes and the CRC checks.
pub fn decode_fich(conv: &mut ConvChannel, soft: &[f32]) -> Option<(Fich, usize)> {
    let c = deinterleave(soft.get(..200)?)?;
    let (g, errs) = conv.decode(&c, 100, true).ok()?;
    let mut b = Vec::with_capacity(48);
    for k in 0..4 {
        let (cw, _) = FICH_WORD.decode(to_u64(&g[k * 24..k * 24 + 24]) as u128)?;
        push_u64(&mut b, FICH_WORD.info(cw) as u64, 12);
    }
    CRC16_GSM
        .verify_bits(&b)
        .then(|| (Fich::from_bits(&b), errs))
}

/// A DCH unit of 10 data bytes (200 bits, V/D mode 2) or 20 (360 bits, the other frames): the data whitened, a CRC-16
/// over the whitened bits, 4 tail bits, rate 1/2, the dibit interleave of its length. Empty for another length.
pub fn encode_dch(data: &[u8]) -> Vec<u8> {
    if data.len() != 10 && data.len() != 20 {
        return Vec::new();
    }
    let mut b = bytes_to_bits(data);
    whitening().apply_bits(&mut b);
    let crc = CRC16_GSM.bits(&b);
    push_u64(&mut b, crc as u64, 16);
    b.extend([0, 0, 0, 0]);
    let mut c = Vec::with_capacity(2 * b.len());
    channel().encode(&b, &mut c);
    interleave(&c)
}

/// Decodes the 200 or 360 soft values of a DCH unit: its 10 or 20 data bytes and the bits repaired, when the CRC checks.
pub fn decode_dch(conv: &mut ConvChannel, soft: &[f32]) -> Option<(Vec<u8>, usize)> {
    let c = deinterleave(soft)?;
    let steps = soft.len() / 2;
    let n = steps - 20;
    let (b, errs) = conv.decode(&c, steps, true).ok()?;
    if !CRC16_GSM.verify_bits(&b[..n + 16]) {
        return None;
    }
    let mut d = b[..n].to_vec();
    whitening().apply_bits(&mut d);
    Some((bits_to_bytes(&d), errs))
}

/// The V/D mode 2 interleave of a voice unit (26 by 4): S_i is sent at channel position 4 (i mod 26) + floor(i / 26).
static VD2_INTERLEAVE: LazyLock<Permutation> =
    LazyLock::new(|| Permutation::from_fn(104, |i| 4 * (i % 26) + i / 26).expect("a permutation"));

/// The 104 channel bits (VCH then VeCH) of a V/D mode 2 voice unit for its 49 speech bits u0 to u3: each of the first
/// 27 three times in a row, the other 22 once, one zero bit; whitened, interleaved 26 by 4.
pub fn vd2_channel(speech: &[u8; 49]) -> [u8; 104] {
    let mut s = [0u8; 104];
    for q in 0..27 {
        s[3 * q..3 * q + 3].fill(speech[q] & 1);
    }
    for (d, &b) in s[81..103].iter_mut().zip(&speech[27..]) {
        *d = b & 1;
    }
    whitening().apply_bits(&mut s);
    let mut out = [0u8; 104];
    VD2_INTERLEAVE.apply(&s, &mut out).expect("104 bits");
    out
}

/// The 49 speech bits u0 to u3 of the 104 channel bits of a V/D mode 2 voice unit, packed as the `info49` payload (most
/// significant bit first, 7 zero bits of padding), and the unit's error measure ([`vd2_errors`]). De-interleaves,
/// removes the whitening and takes the majority of the three copies of each of the first 27 bits.
pub fn vd2_speech(ch: &[u8; 104]) -> ([u8; 7], u32) {
    let mut s = [0u8; 104];
    VD2_INTERLEAVE.invert(ch, &mut s).expect("104 bits");
    whitening().apply_bits(&mut s);
    let mut info = [0u8; 7];
    for q in 0..49 {
        let b = if q < 27 {
            u8::from(s[3 * q] + s[3 * q + 1] + s[3 * q + 2] >= 2)
        } else {
            s[54 + q]
        };
        info[q / 8] |= b << (7 - q % 8);
    }
    (info, vd2_errors(&s))
}

/// The error measure of a de-whitened V/D mode 2 voice unit (unconfirmed): the triples whose copies differ, plus 1 when
/// the added zero bit S_103 reads 1.
fn vd2_errors(s: &[u8; 104]) -> u32 {
    let split = s[..81]
        .as_chunks::<3>()
        .0
        .iter()
        .filter(|t| t[0] != t[1] || t[1] != t[2])
        .count() as u32;
    split + u32::from(s[103])
}

/// The highest error measure of a V/D mode 2 voice unit that still goes to the speech decoder.
const VD2_LIMIT: u32 = 7;

/// Whether a V/D mode 2 voice unit of error measure `errors` ([`vd2_errors`]) is lost (unconfirmed): above
/// [`VD2_LIMIT`], a quarter of its 28 checks.
fn vd2_lost(errors: u32) -> bool {
    errors > VD2_LIMIT
}

/// Whether a voice full-rate communication frame is the sub-header frame CC0 (unconfirmed): the first communication
/// frame after a header, or a frame whose FICH has FT = 1 and FN = 0.
fn is_voice_fr_sub_header(first_after_header: bool, f: &Fich) -> bool {
    first_after_header || (f.ft == 1 && f.fn_ == 0)
}

/// A run of equal pieces in the 720 bits after the FICH (bit 0 is frame bit 240): piece k is `len` bits from
/// `start + k stride`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Part {
    start: usize,
    stride: usize,
    len: usize,
    count: usize,
}

impl Part {
    const fn new(start: usize, stride: usize, len: usize, count: usize) -> Part {
        Part {
            start,
            stride,
            len,
            count,
        }
    }

    /// The place of piece `k`.
    fn range(&self, k: usize) -> Range<usize> {
        let a = self.start + k * self.stride;
        a..a + self.len
    }

    /// The pieces of `payload`, in order.
    fn gather<T: Copy>(&self, payload: &[T]) -> Vec<T> {
        (0..self.count)
            .flat_map(|k| payload[self.range(k)].iter().copied())
            .collect()
    }

    /// Writes `bits` into the pieces of `payload`.
    fn scatter(&self, bits: &[u8], payload: &mut [u8]) {
        for (k, c) in bits.chunks(self.len).take(self.count).enumerate() {
            payload[self.range(k)][..c.len()].copy_from_slice(c);
        }
    }
}

/// DCH-1: the first 72 bits of each sub-frame.
const DCH_1: Part = Part::new(0, 144, 72, 5);
/// DCH-2: the last 72 bits of each sub-frame.
const DCH_2: Part = Part::new(72, 144, 72, 5);
/// The DCH of V/D mode 2: the first 40 bits of each sub-frame.
const DCH_VD2: Part = Part::new(0, 144, 40, 5);
/// The DCH of the voice full-rate sub-header frame: frame bits 240 to 599.
const DCH_SUB: Part = Part::new(0, 72, 72, 5);

/// The layout of a frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    /// A header or terminator frame: DCH-1 and DCH-2.
    HeaderTerminator,
    /// V/D mode 1: DCH then VCH (72) in each sub-frame.
    Vd1,
    /// Data full rate: DCH-1 and DCH-2.
    DataFr,
    /// V/D mode 2: DCH (40), VCH (72), VeCH (32) in each sub-frame.
    Vd2,
    /// Voice full rate: VCH (144) in each sub-frame.
    VoiceFr,
    /// The voice full-rate sub-header frame CC0: a 360-bit DCH unit, 72 reserved bits, voice frames 3 and 4.
    VoiceFrSub,
}

impl Kind {
    /// The layout of a frame of FICH `f`; `first_after_header` when it follows a header frame. None for a test frame.
    fn of(f: &Fich, first_after_header: bool) -> Option<Kind> {
        match (f.fi, f.dt) {
            (0 | 2, _) => Some(Kind::HeaderTerminator),
            (1, 0) => Some(Kind::Vd1),
            (1, 1) => Some(Kind::DataFr),
            (1, 2) => Some(Kind::Vd2),
            (1, _) if is_voice_fr_sub_header(first_after_header, f) => Some(Kind::VoiceFrSub),
            (1, _) => Some(Kind::VoiceFr),
            _ => None,
        }
    }

    /// The voice layout of data type `dt` in a frame of standard layout; None for data FR.
    fn of_voice(dt: u8) -> Option<Kind> {
        match dt {
            0 => Some(Kind::Vd1),
            2 => Some(Kind::Vd2),
            3 => Some(Kind::VoiceFr),
            _ => None,
        }
    }

    /// The DCH units, in order.
    fn dch(self) -> &'static [Part] {
        match self {
            Kind::HeaderTerminator | Kind::DataFr => &[DCH_1, DCH_2],
            Kind::Vd1 => &[DCH_1],
            Kind::Vd2 => &[DCH_VD2],
            Kind::VoiceFr => &[],
            Kind::VoiceFrSub => &[DCH_SUB],
        }
    }

    /// The voice frames and the number (0 to 4) of the first one in the 100 ms of the frame.
    fn voice(self) -> Option<(Part, usize)> {
        match self {
            Kind::Vd1 => Some((Part::new(72, 144, 72, 5), 0)),
            Kind::Vd2 => Some((Part::new(40, 144, 104, 5), 0)),
            Kind::VoiceFr => Some((Part::new(0, 144, 144, 5), 0)),
            Kind::VoiceFrSub => Some((Part::new(432, 144, 144, 2), 3)),
            Kind::HeaderTerminator | Kind::DataFr => None,
        }
    }

    /// The codec and packaging of the voice frames: V/D mode 1 the half-rate code words, V/D mode 2 the 49 speech bits,
    /// voice full rate the 144 channel bits as P25 Phase 1 sends them.
    fn speech(self) -> Option<(Codec, SpeechPackaging)> {
        match self {
            Kind::Vd1 => Some((Codec::Ambe2, SpeechPackaging::Codewords)),
            Kind::Vd2 => Some((Codec::Ambe2, SpeechPackaging::Info49)),
            Kind::VoiceFr | Kind::VoiceFrSub => Some((Codec::Imbe, SpeechPackaging::P25_144)),
            Kind::HeaderTerminator | Kind::DataFr => None,
        }
    }

    /// Whether the DCH carries the text and GPS of the call.
    fn carries_text(self) -> bool {
        matches!(self, Kind::Vd1 | Kind::Vd2 | Kind::DataFr)
    }
}

#[derive(Clone, Debug, Default)]
struct Call {
    dest: String,
    src: String,
    down: String,
    up: String,
    dt: u8,
    blocks: Vec<u8>,
}

/// The two DCH units of a header or terminator frame: DCH-1 the destination then the source, DCH-2 the downlink then
/// the uplink, 10 characters each, padded with spaces. The specification used here does not give their content.
fn call_sign_units(c: &Call) -> [[u8; 20]; 2] {
    let ten = |s: &str, out: &mut [u8]| {
        out.fill(b' ');
        for (d, b) in out.iter_mut().zip(s.bytes()) {
            *d = b;
        }
    };
    let mut u = [[0u8; 20]; 2];
    ten(&c.dest, &mut u[0][..10]);
    ten(&c.src, &mut u[0][10..]);
    ten(&c.down, &mut u[1][..10]);
    ten(&c.up, &mut u[1][10..]);
    u
}

/// Reads the call signs of the decoded DCH units of a header or terminator frame into `c` (the layout of
/// [`call_sign_units`]).
fn read_call_signs(units: &[Option<Vec<u8>>; 2], c: &mut Call) {
    let text = |b: &[u8]| String::from_utf8_lossy(b).trim_end().to_string();
    if let Some(u) = &units[0] {
        c.dest = text(&u[..10]);
        c.src = text(&u[10..]);
    }
    if let Some(u) = &units[1] {
        c.down = text(&u[..10]);
        c.up = text(&u[10..]);
    }
}

/// The System Fusion protocol.
pub struct Ysf {
    buf: Vec<Sym>,
    active: bool,
    idle: usize,
    call: Call,
    conv: ConvChannel,
    /// The last frame read was a header.
    after_header: bool,
}

impl Default for Ysf {
    fn default() -> Self {
        Ysf {
            buf: Vec::with_capacity(480),
            active: false,
            idle: 0,
            call: Call::default(),
            conv: channel(),
            after_header: false,
        }
    }
}

const KEY: u32 = 1;

fn dt_name(dt: u8) -> &'static str {
    match dt {
        0 => "V/D mode 1",
        1 => "data FR",
        2 => "V/D mode 2",
        _ => "voice FR",
    }
}

/// The hard bits of soft values.
fn hard(s: &[f32], out: &mut [u8]) {
    for (b, &v) in out.iter_mut().zip(s) {
        *b = u8::from(v > 0.0);
    }
}

impl Ysf {
    fn frame(&mut self, ctx: &mut Ctx) {
        let buf = std::mem::take(&mut self.buf);
        let mut soft = Vec::with_capacity(920);
        for s in &buf[20..480] {
            let (a, b) = s.soft_dibit();
            soft.push(a);
            soft.push(b);
        }
        self.buf = buf;
        self.buf.clear();
        let fich = decode_fich(&mut self.conv, &soft[..200]);
        ctx.identity(fich.is_some());
        ctx.block(fich.is_some(), 200, fich.map_or(0, |f| f.1));
        let Some((fich, _)) = fich else {
            self.lost(ctx);
            return;
        };
        let first_after_header = std::mem::replace(&mut self.after_header, fich.fi == 0);
        let Some(kind) = Kind::of(&fich, first_after_header) else {
            return;
        };
        let payload = &soft[200..920];
        let mut units: [Option<Vec<u8>>; 2] = [None, None];
        for (u, part) in units.iter_mut().zip(kind.dch()) {
            let s = part.gather(payload);
            let d = decode_dch(&mut self.conv, &s);
            ctx.block(d.is_some(), s.len(), d.as_ref().map_or(0, |x| x.1));
            *u = d.map(|x| x.0);
        }
        match fich.fi {
            0 => {
                read_call_signs(&units, &mut self.call);
                if !self.call.src.is_empty() && !ctx.has_call(KEY) {
                    let mut rec = CallRecord {
                        kind: CallKind::Voice,
                        src: Ident::Text(self.call.src.clone()),
                        dst: Ident::Text(self.call.dest.clone()),
                        group: fich.cm == 0,
                        encryption: Encryption::clear(),
                        access: Some(Access::new(AccessKind::SquelchCode, u32::from(fich.sc))),
                        ..Default::default()
                    };
                    rec.set(CallDetail::Downlink, Value::Text(self.call.down.clone()));
                    rec.set(CallDetail::Uplink, Value::Text(self.call.up.clone()));
                    rec.set(CallDetail::DataType, Value::Text(dt_name(fich.dt).into()));
                    self.call.dt = fich.dt;
                    self.call.blocks.clear();
                    ctx.start_call(KEY, rec, false);
                    ctx.set_encryption(KEY, Encryption::clear());
                }
            }
            1 => {
                if !ctx.has_call(KEY) {
                    return;
                }
                self.call.dt = fich.dt;
                self.speech(kind, payload, ctx);
                if kind.carries_text() {
                    if fich.fn_ == 0 {
                        self.call.blocks.clear();
                    }
                    let n = kind.dch().len();
                    let got: Vec<&Vec<u8>> = units.iter().flatten().collect();
                    if got.len() == n {
                        if fich.fn_ < 8 {
                            got.iter().for_each(|u| self.call.blocks.extend_from_slice(u));
                        }
                        if fich.fn_ == fich.ft {
                            self.text(ctx);
                        }
                    }
                }
            }
            _ => {
                if ctx.has_call(KEY) {
                    self.text(ctx);
                    ctx.end_call(KEY, true);
                }
                self.call = Call::default();
            }
        }
    }

    /// The voice frames of a communication frame of layout `kind` as speech frames of the call.
    fn speech(&mut self, kind: Kind, payload: &[f32], ctx: &mut Ctx) {
        let (Some((part, first)), Some(spec)) = (kind.voice(), speech_spec(kind, ctx.carrier_hz)) else {
            return;
        };
        ctx.update_call(KEY, |c| c.voice_frames += part.count as u32);
        let mut bits = [0u8; 144];
        for k in 0..part.count {
            let t = ctx.now + (first + k) as f64 * 0.02;
            let b = &mut bits[..part.len];
            hard(&payload[part.range(k)], b);
            match kind {
                Kind::Vd1 => {
                    let cw = sdrkit_codec_mbe::codewords(b);
                    ctx.speech(KEY, spec, &cw, t, false, None);
                }
                Kind::Vd2 => {
                    let (info, errors) = vd2_speech((&*b).try_into().expect("104 bits"));
                    if vd2_lost(errors) {
                        ctx.voice(KEY, crate::voice::erasure(spec, t));
                    } else {
                        ctx.speech(KEY, spec, &info, t, false, None);
                    }
                }
                _ => {
                    let cb = crate::voice::channel_bits::<18>(b);
                    ctx.speech(KEY, spec, &cb, t, false, None);
                }
            }
        }
    }

    /// A frame whose FICH did not decode inside a call: its five voice frames, of the data type of the call, are lost.
    fn lost(&mut self, ctx: &mut Ctx) {
        if !ctx.has_call(KEY) {
            return;
        }
        let Some(kind) = Kind::of_voice(self.call.dt) else {
            return;
        };
        let Some(spec) = speech_spec(kind, ctx.carrier_hz) else {
            return;
        };
        for k in 0..5 {
            ctx.voice(KEY, crate::voice::erasure(spec, ctx.now + k as f64 * 0.02));
        }
    }

    /// The data channel's content: a text message ("T" and the text) or a GPS position ("G" lat lon in 1e-5 degrees).
    fn text(&mut self, ctx: &mut Ctx) {
        let b = std::mem::take(&mut self.call.blocks);
        if b.is_empty() {
            return;
        }
        match b[0] {
            b'T' => {
                let text = String::from_utf8_lossy(&b[1..])
                    .trim_end_matches(['\0', ' '])
                    .to_string();
                ctx.update_call(KEY, |c| c.text = text.clone());
                ctx.message(MessageRecord {
                    src: Ident::Text(self.call.src.clone()),
                    dst: Ident::Text(self.call.dest.clone()),
                    format: MessageFormat::YsfText,
                    data_format: None,
                    text,
                    crc_ok: true,
                    time: ctx.now,
                    ..Default::default()
                });
            }
            b'G' if b.len() >= 9 => {
                let lat = i32::from_be_bytes([b[1], b[2], b[3], b[4]]) as f64 * 1e-5;
                let lon = i32::from_be_bytes([b[5], b[6], b[7], b[8]]) as f64 * 1e-5;
                ctx.position(PositionRecord {
                    src: Ident::Text(self.call.src.clone()),
                    lat,
                    lon,
                    source: PositionFormat::YsfGps,
                    time: ctx.now,
                    ..Default::default()
                });
            }
            _ => {}
        }
    }
}

/// The speech frame description of the voice frames of layout `kind` on `carrier_hz`.
fn speech_spec(kind: Kind, carrier_hz: f64) -> Option<crate::voice::FrameSpec> {
    let (codec, packaging) = kind.speech()?;
    Some(crate::voice::FrameSpec {
        codec,
        packaging,
        codec_mode: None,
        duration_ms: 20,
        stack: "ysf",
        carrier_hz,
        slot: 0,
    })
}

impl Protocol for Ysf {
    fn params(&self) -> ModeParams {
        ModeParams {
            mode: "ysf",
            name: "Yaesu System Fusion",
            baud: 4800.0,
            two_level: false,
            channel_hz: 12_500.0,
            calls: &crate::results::CALLS_300,
            message_cap: 100,
            polarity_rule: false,
            clock_loop: true,
            extra_baud: None,
            stream_rate: crate::front::RATE,
        }
    }

    fn patterns(&self) -> Vec<SyncPattern> {
        vec![SyncPattern::from_hex_dibits(0, FSW, 0).with_max_err(2)]
    }

    fn on_sync(
        &mut self,
        _id: u8,
        _inv: bool,
        pre: &[Sym],
        _at: u64,
        tracking: bool,
        _ctx: &mut Ctx,
    ) -> SyncAnswer {
        if tracking && self.active && self.buf.len() > 25 && self.buf.len() < 475 {
            return SyncAnswer::Ignore;
        }
        self.buf.clear();
        self.buf.extend_from_slice(pre);
        self.active = true;
        self.idle = 0;
        SyncAnswer::Take
    }

    fn on_symbol(&mut self, s: Sym, _at: u64, ctx: &mut Ctx) -> SymAnswer {
        if !self.active {
            self.idle += 1;
            return if self.idle > 1000 {
                SymAnswer::Stop
            } else {
                SymAnswer::Continue
            };
        }
        self.buf.push(s);
        if self.buf.len() == 480 {
            if crate::symbols::carrier_gone(&self.buf) {
                self.active = false;
                return SymAnswer::Stop;
            }
            self.frame(ctx);
            self.active = false;
        }
        SymAnswer::Continue
    }

    fn on_tick(&mut self, ctx: &mut Ctx) {
        if ctx.has_call(KEY) && ctx.now - ctx.last_frame_s > 1.0 {
            ctx.end_call(KEY, false);
        }
    }

    fn fields(&self) -> Vec<&'static dyn Declare> {
        <YsfFields as Fields>::SHAPE.fields.to_vec()
    }

    fn publish(&self, r: &mut Published) {
        r.put(&YsfFields {
            data_type: dt_name(self.call.dt),
        });
    }

    fn codec(&self) -> Codec {
        if self.call.dt == 3 {
            Codec::Imbe
        } else {
            Codec::Ambe2
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------------------
// Generator.

sdrkit_core::choice_enum! {
    /// The voice data type of the test signal.
    pub enum YsfVoice {
        /// V/D mode 1: half-rate speech with its own error correction, and the data channel.
        Vd1 = ("vd1", "V/D mode 1"),
        /// V/D mode 2: half-rate speech bits sent three times, and the data channel.
        Vd2 = ("vd2", "V/D mode 2"),
        /// Voice full rate: full-rate speech only.
        VoiceFr = ("voice_fr", "Voice full rate"),
    }
}

impl YsfVoice {
    /// The FICH data type.
    fn dt(self) -> u8 {
        match self {
            YsfVoice::Vd1 => 0,
            YsfVoice::Vd2 => 2,
            YsfVoice::VoiceFr => 3,
        }
    }
}

/// The voice data type of the test signal.
pub const GEN_VOICE: Setting<field::Choice<YsfVoice>> =
    Setting::choice("gen_voice", "Voice type", YsfVoice::Vd2)
        .section("Test signal")
        .help("The data type of the voice of the test signal: V/D mode 1, V/D mode 2 or voice full rate.")
        .advanced();

/// The channel bits of voice frame `n` of the test speech for layout `kind`, as its VCH (and VeCH) carries them.
fn test_voice(kind: Kind, n: usize) -> Vec<u8> {
    match kind {
        Kind::Vd1 => {
            let s = crate::voice::test_speech_half();
            s[n % s.len()].to_vec()
        }
        Kind::Vd2 => {
            let s = crate::voice::test_speech_half_bits();
            vd2_channel(&s[n % s.len()]).to_vec()
        }
        _ => {
            let s = crate::voice::test_speech_full();
            s[n % s.len()].to_vec()
        }
    }
}

/// The 960 bits of a frame of layout `kind`: the sync, the FICH `f`, the coded DCH `units` and the `voice` frames at
/// their places. Bits with no content are zero.
fn frame_bits(f: &Fich, kind: Kind, units: &[Vec<u8>], voice: &[Vec<u8>]) -> Vec<u8> {
    let mut b = Vec::with_capacity(960);
    push_u64(&mut b, u64::from_str_radix(FSW, 16).unwrap_or(0), 40);
    b.extend(encode_fich(f));
    let mut payload = [0u8; 720];
    for (part, unit) in kind.dch().iter().zip(units) {
        part.scatter(unit, &mut payload);
    }
    if let Some((part, _)) = kind.voice() {
        part.scatter(&voice.concat(), &mut payload);
    }
    b.extend_from_slice(&payload);
    b
}

/// A transmission of `voice`: two header frames, `frames` communication frames with the test speech and, in the V/D
/// modes, a text or GPS in the data channel, and a terminator frame. In voice full rate the first communication frame
/// is the sub-header frame CC0 (FN 0, FT 1) and the others have FN 0, FT 0.
pub fn transmission(
    src: &str,
    dest: &str,
    text: &str,
    gps: Option<(f64, f64)>,
    frames: usize,
    voice: YsfVoice,
) -> Vec<u8> {
    let dt = voice.dt();
    let call = Call {
        dest: dest.into(),
        src: src.into(),
        down: "DOWNLINK".into(),
        up: "UPLINK".into(),
        ..Default::default()
    };
    let signs: Vec<Vec<u8>> = call_sign_units(&call)
        .iter()
        .map(|u| encode_dch(u))
        .collect();
    let mut b = Vec::new();
    for fn_ in 0..2u8 {
        let f = Fich {
            fi: 0,
            cs: 2,
            fn_,
            ft: 1,
            dt,
            ..Default::default()
        };
        b.extend(frame_bits(&f, Kind::HeaderTerminator, &signs, &[]));
    }
    let mut data = Vec::new();
    if let Some((lat, lon)) = gps {
        data.push(b'G');
        data.extend_from_slice(&((lat * 1e5).round() as i32).to_be_bytes());
        data.extend_from_slice(&((lon * 1e5).round() as i32).to_be_bytes());
    } else {
        data.push(b'T');
        data.extend_from_slice(text.as_bytes());
    }
    let block = if dt == 2 { 10 } else { 20 };
    let blocks: Vec<Vec<u8>> = data
        .chunks(block)
        .map(|c| {
            let mut a = vec![0u8; block];
            a[..c.len()].copy_from_slice(c);
            a
        })
        .collect();
    let ft = (blocks.len() - 1) as u8;
    let mut n = 0;
    for k in 0..frames {
        let (f, kind, units) = if dt == 3 {
            let sub = k == 0;
            let f = Fich {
                fi: 1,
                cs: 2,
                ft: u8::from(sub),
                dt,
                ..Default::default()
            };
            let units = if sub { vec![encode_dch(&[b' '; 20])] } else { vec![] };
            (f, if sub { Kind::VoiceFrSub } else { Kind::VoiceFr }, units)
        } else {
            let fn_ = (k % blocks.len()) as u8;
            let f = Fich {
                fi: 1,
                cs: 2,
                fn_,
                ft,
                dt,
                ..Default::default()
            };
            let kind = Kind::of_voice(dt).expect("a voice type");
            (f, kind, vec![encode_dch(&blocks[fn_ as usize])])
        };
        let count = kind.voice().map_or(0, |(p, _)| p.count);
        let speech: Vec<Vec<u8>> = (n..n + count).map(|i| test_voice(kind, i)).collect();
        n += count;
        b.extend(frame_bits(&f, kind, &units, &speech));
    }
    let f = Fich {
        fi: 2,
        cs: 2,
        dt,
        ..Default::default()
    };
    b.extend(frame_bits(&f, Kind::HeaderTerminator, &signs, &[]));
    b
}

/// The signal of the transmissions `bits` (frame bits of [`transmission`]), 0.6 s of silence after each.
pub fn render_bits(bits: &[Vec<u8>]) -> Vec<Complex32> {
    let mut m = FskModulator::new(4800.0, TxPulse::RaisedCosine(0.2), 900.0);
    let mut out = Vec::new();
    silence(0.3, &mut out);
    for b in bits {
        m.symbols(&bits_to_levels(b), &mut out);
        m.flush(8, &mut out);
        m.reset();
        silence(0.6, &mut out);
    }
    out
}

/// The System Fusion test signal of `voice`: a call with a text and a call with a GPS position.
pub fn render(voice: YsfVoice) -> Vec<Complex32> {
    render_bits(&[
        transmission("N0CALL", "ALL", "Fusion test one", None, 20, voice),
        transmission("N1TEST", "ALL", "", Some((48.85837, 2.29448)), 12, voice),
    ])
}

/// The System Fusion generator from the settings of the stack ([`GEN_VOICE`]).
pub fn generator(cfg: &GenConfig) -> Prepared {
    Prepared::new(render(cfg.settings.get(&GEN_VOICE)), cfg.rate_hz, true)
        .with_info(12_500.0, 4800.0, 9600.0)
}

#[cfg(test)]
mod tests;
