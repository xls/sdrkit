//! dPMR and PMR446 digital: the frame syncs, the header with the colour code, superframes of four 80 ms frames (control
//! channel and four AMBE+2 voice frames each), status, short messages and packet data.
//! Standard: ETSI TS 102 490, ETSI TS 102 658.

use crate::bits::{bits_to_bytes, bytes_to_bits};
use crate::decoder::{Ctx, ModeParams, Protocol};
use crate::gen::{bits_to_levels, silence, FskModulator, Prepared, TxPulse};
use crate::items::*;
use crate::symbols::{Sym, SymAnswer, SyncAnswer, SyncPattern};
use crate::typed::DpmrFields;
use sdrkit_algo_fec::small::SmallCode;
use sdrkit_algo_fec::voice::{dpmr_hi_interleave, dpmr_tch_interleave};
use sdrkit_core::interface::Fields;
use sdrkit_core::media::Codec;
use sdrkit_core::publish::Published;
use sdrkit_core::publish::Declare;
use sdrkit_core::{Complex32, GenConfig, Value};
use sdrkit_util_bits::crc::catalog::{CRC7_MMC, CRC8_SMBUS};
use sdrkit_util_bits::interleave::Permutation;
use sdrkit_util_bits::lfsr::Additive;
use sdrkit_util_bits::pack::{push_u64, to_u64};
use sdrkit_util_bits::seq;
use std::sync::LazyLock;

pub const FS1: &str = "57FF5F75D577";
pub const FS2: &str = "5FF7D7";
pub const FS3: &str = "7DFFD5F55D5F";
pub const FS4: &str = "FD55F5DF7FDD";

/// Hamming (12,8) of the coded blocks.
static WORD: LazyLock<SmallCode> = LazyLock::new(SmallCode::hamming_12_8);
/// Extended Golay (24,12) of the colour code word.
static CC_WORD: LazyLock<SmallCode> = LazyLock::new(SmallCode::golay_24_12);

/// The interleaves of the coded blocks: the header (120 bits, HI interleave), the control channel (72 bits, TCH interleave),
/// and a data frame (288 bits, the TCH interleave on each 72-bit part).
static HI: LazyLock<Permutation> = LazyLock::new(dpmr_hi_interleave);
static TCH: LazyLock<Permutation> = LazyLock::new(dpmr_tch_interleave);

/// Interleaves (`back` false) or de-interleaves a coded block of 120, 72 or 288 bits. False for another size.
fn interleave(input: &[u8], out: &mut [u8], back: bool) -> bool {
    let run = |p: &Permutation, i: &[u8], o: &mut [u8]| {
        if back {
            p.invert(i, o).is_ok()
        } else {
            p.apply(i, o).is_ok()
        }
    };
    match input.len() {
        120 => run(&HI, input, out),
        72 | 288 => input
            .chunks(72)
            .zip(out.chunks_mut(72))
            .all(|(i, o)| run(&TCH, i, o)),
        _ => false,
    }
}

/// XORs the dPMR scrambler (x^9 + x^5 + 1, started at 0x100 for each block) onto `bits`.
fn scramble(bits: &mut [u8]) {
    Additive::new(seq::DPMR).apply_bits(bits);
}

/// Encodes `info` (80, 48 or 192 bits) with Hamming (12,8) words, the 6-row interleave and the scrambler. An empty result for
/// another size.
pub fn encode_block(info: &[u8]) -> Vec<u8> {
    let n = info.len() / 8 * 12;
    if !info.len().is_multiple_of(8) {
        return Vec::new();
    }
    let mut coded = Vec::with_capacity(n);
    for c in info.chunks(8) {
        push_u64(&mut coded, WORD.encode(to_u64(c) as u128) as u64, 12);
    }
    let mut out = vec![0u8; n];
    if !interleave(&coded, &mut out, false) {
        return Vec::new();
    }
    scramble(&mut out);
    out
}

/// Decodes a block of 120, 72 or 288 bits: the information, the bits repaired, and whether every word decoded.
pub fn decode_block(bits: &[u8]) -> (Vec<u8>, usize, bool) {
    let n = bits.len();
    let mut info = Vec::with_capacity(n / 12 * 8);
    let mut d = bits.to_vec();
    scramble(&mut d);
    let mut coded = vec![0u8; n];
    if !interleave(&d, &mut coded, true) {
        return (info, 0, false);
    }
    let mut fixed = 0;
    let mut ok = true;
    for w in coded.chunks(12) {
        match WORD.decode(to_u64(w) as u128) {
            Some((cw, e)) => {
                fixed += e as usize;
                push_u64(&mut info, WORD.info(cw) as u64, 8);
            }
            None => {
                ok = false;
                push_u64(&mut info, to_u64(&w[..8]), 8);
            }
        }
    }
    (info, fixed, ok)
}

/// The 24-bit colour code word of colour code `cc` (0..63): the extended Golay word of the code repeated twice.
pub fn cc_word(cc: u8) -> u32 {
    let c = cc as u128 & 0x3F;
    CC_WORD.encode((c << 6) | c) as u32
}

/// The colour code of a received word.
pub fn cc_of(word: u32) -> Option<u8> {
    let (cw, _) = CC_WORD.decode(word as u128)?;
    let d = CC_WORD.info(cw) as u32;
    ((d >> 6) == (d & 0x3F)).then_some((d & 0x3F) as u8)
}

/// Communication modes of the header.
pub fn comm_mode_name(m: u8) -> &'static str {
    match m {
        0 => "voice",
        1 => "data type 1",
        2 => "data type 2",
        3 => "data type 3",
        4 => "voice and data",
        5 => "packet data",
        _ => "reserved",
    }
}

/// The header information: called and own IDs, communication mode, format, emergency. 80 bits with CRC-7.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HeaderInfo {
    pub called: u32,
    pub own: u32,
    pub comm_mode: u8,
    /// 0 individual, 1 group, 2 all call.
    pub format: u8,
    pub emergency: bool,
}

impl HeaderInfo {
    pub fn bits(&self) -> Vec<u8> {
        let mut b = Vec::with_capacity(80);
        push_u64(&mut b, 0, 2);
        push_u64(&mut b, self.called as u64 & 0xFF_FFFF, 24);
        push_u64(&mut b, self.own as u64 & 0xFF_FFFF, 24);
        push_u64(&mut b, self.comm_mode as u64, 3);
        push_u64(&mut b, self.format as u64, 4);
        push_u64(&mut b, self.emergency as u64, 1);
        push_u64(&mut b, 0, 15);
        let crc = CRC7_MMC.bits(&b);
        push_u64(&mut b, crc as u64, 7);
        b
    }
    pub fn from_bits(b: &[u8]) -> Option<HeaderInfo> {
        if b.len() < 80 || CRC7_MMC.bits(&b[..73]) as u64 != to_u64(&b[73..80]) {
            return None;
        }
        Some(HeaderInfo {
            called: to_u64(&b[2..26]) as u32,
            own: to_u64(&b[26..50]) as u32,
            comm_mode: to_u64(&b[50..53]) as u8,
            format: to_u64(&b[53..57]) as u8,
            emergency: b[57] != 0,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum State {
    Idle,
    Header(bool),
    Frames,
    End,
}

/// The dPMR protocol.
pub struct Dpmr {
    state: State,
    buf: Vec<Sym>,
    hdr: Option<HeaderInfo>,
    cc: Option<u8>,
    frame_no: usize,
    data: Vec<u8>,
    idle: usize,
    packet: bool,
}

impl Default for Dpmr {
    fn default() -> Self {
        Dpmr {
            state: State::Idle,
            buf: Vec::with_capacity(400),
            hdr: None,
            cc: None,
            frame_no: 0,
            data: Vec::new(),
            idle: 0,
            packet: false,
        }
    }
}

const KEY: u32 = 1;

fn bits_of(s: &[Sym]) -> Vec<u8> {
    let mut b = Vec::with_capacity(s.len() * 2);
    for x in s {
        let d = x.dibit();
        b.push(d >> 1);
        b.push(d & 1);
    }
    b
}

fn id_text(id: u32) -> Ident {
    if id == 0xFF_FFFF {
        Ident::Text("ALL (FFFFFF)".into())
    } else {
        Ident::Number(id as u64)
    }
}

impl Dpmr {
    fn header(&mut self, ctx: &mut Ctx) {
        let b = bits_of(&self.buf);
        let mut best = None;
        for part in [&b[..120], &b[144..264]] {
            let (info, fixed, ok) = decode_block(part);
            let h = if ok {
                HeaderInfo::from_bits(&info)
            } else {
                None
            };
            ctx.block(h.is_some(), 120, fixed);
            if best.is_none() {
                best = h;
            }
        }
        let cc = cc_of(to_u64(&b[120..144]) as u32);
        ctx.identity(best.is_some());
        self.cc = cc.or(self.cc);
        let Some(h) = best else {
            self.state = State::Idle;
            return;
        };
        self.hdr = Some(h);
        self.data.clear();
        self.frame_no = 0;
        self.state = State::Frames;
        if h.comm_mode == 0 || h.comm_mode == 4 {
            if ctx.has_call(KEY) {
                return;
            }
            let rec = self.record(&h, CallKind::Voice);
            ctx.start_call(KEY, rec, false);
            ctx.set_encryption(KEY, Encryption::clear());
        }
    }

    fn record(&self, h: &HeaderInfo, kind: CallKind) -> CallRecord {
        let mut rec = CallRecord {
            kind,
            src: Ident::Number(h.own as u64),
            dst: id_text(h.called),
            group: h.format != 0,
            emergency: h.emergency,
            access: self
                .cc
                .map(|c| Access::new(AccessKind::ColourCode, u32::from(c))),
            encryption: Encryption::clear(),
            ..Default::default()
        };
        let comm = if self.packet {
            "packet data"
        } else {
            comm_mode_name(h.comm_mode)
        };
        rec.set(CallDetail::CommMode, Value::Text(comm.into()));
        let call_type = match h.format {
            0 => "individual",
            1 => "group",
            _ => "all call",
        };
        rec.set(CallDetail::CallType, Value::Text(call_type.into()));
        rec
    }

    fn payload_frame(&mut self, ctx: &mut Ctx) {
        let b = bits_of(&self.buf);
        // Control channel: frame number (2), the half of the called ID (12) ... with CRC-8.
        let (cch, fixed, ok) = decode_block(&b[24..96]);
        let crc_ok = ok && CRC8_SMBUS.bits(&cch[..40]) as u64 == to_u64(&cch[40..48]);
        ctx.block(crc_ok, 72, fixed);
        ctx.identity(crc_ok);
        if self.frame_no % 2 == 1 {
            if let Some(cc) = cc_of(to_u64(&b[..24]) as u32) {
                self.cc = Some(cc);
            }
        }
        self.frame_no += 1;
        let Some(h) = self.hdr else {
            return;
        };
        let tch = &b[96..384];
        if h.comm_mode == 0 || h.comm_mode == 4 {
            if ctx.has_call(KEY) {
                ctx.update_call(KEY, |c| c.voice_frames += 4);
                for k in 0..4 {
                    let cw = sdrkit_codec_mbe::codewords(&tch[k * 72..k * 72 + 72]);
                    let spec = crate::voice::FrameSpec {
                        codec: Codec::Ambe2,
                        packaging: sdrkit_graph::SpeechPackaging::Codewords,
                        codec_mode: None,
                        duration_ms: 20,
                        stack: "dpmr",
                        carrier_hz: ctx.carrier_hz,
                        slot: 0,
                    };
                    ctx.speech(KEY, spec, &cw, ctx.now + k as f64 * 0.02, false, None);
                }
            }
        } else {
            let (d, fixed, ok) = decode_block(tch);
            ctx.block(ok, 288, fixed);
            if ok {
                self.data.extend(bits_to_bytes(&d));
            }
        }
    }

    fn end(&mut self, ctx: &mut Ctx) {
        if let Some(h) = self.hdr.take() {
            if h.comm_mode == 0 || h.comm_mode == 4 {
                ctx.end_call(KEY, true);
            } else if !self.data.is_empty() {
                let len = self.data[0] as usize;
                let body: Vec<u8> = self.data[1..].iter().copied().take(len).collect();
                let text = String::from_utf8_lossy(&body).to_string();
                let (kind, text) = match (self.packet, h.comm_mode) {
                    (true, _) => (CallKind::Packet, text),
                    (_, 1) => (
                        CallKind::Status,
                        format!("status {}", body.first().copied().unwrap_or(0)),
                    ),
                    _ => (CallKind::ShortMessage, text),
                };
                let mut rec = self.record(&h, kind);
                rec.text = text.clone();
                ctx.log_single(rec.clone());
                if kind != CallKind::Status {
                    ctx.message(MessageRecord {
                        src: rec.src,
                        dst: rec.dst,
                        group: rec.group,
                        format: if kind == CallKind::Packet {
                            MessageFormat::Packet
                        } else {
                            MessageFormat::ShortMessage
                        },
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
        self.packet = false;
        self.state = State::Idle;
    }
}

impl Protocol for Dpmr {
    fn params(&self) -> ModeParams {
        ModeParams {
            mode: "dpmr",
            name: "dPMR",
            baud: 2400.0,
            two_level: false,
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
            SyncPattern::from_hex_dibits(1, FS1, 0)
                .with_max_err(3)
                .no_auto_invert(),
            SyncPattern::from_hex_dibits(2, FS2, 0)
                .with_max_err(1)
                .no_auto_invert(),
            SyncPattern::from_hex_dibits(3, FS3, 0)
                .with_max_err(3)
                .no_auto_invert(),
            SyncPattern::from_hex_dibits(4, FS4, 0)
                .with_max_err(3)
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
        match id {
            1 | 4 => {
                if tracking && self.state == State::Frames && self.buf.len() > 20 {
                    return SyncAnswer::Ignore;
                }
                self.state = State::Header(id == 4);
                self.packet = id == 4;
                self.buf.clear();
            }
            2 => {
                if self.state != State::Frames || (self.buf.len() > 4 && self.buf.len() < 186) {
                    return SyncAnswer::Ignore;
                }
                self.buf.clear();
                self.buf.extend_from_slice(pre);
                self.frame_no = 0;
            }
            _ => {
                if self.state != State::Frames {
                    return SyncAnswer::Ignore;
                }
                self.end(ctx);
                self.state = State::End;
            }
        }
        self.idle = 0;
        SyncAnswer::Take
    }

    fn on_symbol(&mut self, s: Sym, _at: u64, ctx: &mut Ctx) -> SymAnswer {
        match self.state {
            State::Idle | State::End => {
                self.idle += 1;
                if self.idle > 600 {
                    return SymAnswer::Stop;
                }
            }
            State::Header(_) => {
                self.buf.push(s);
                if self.buf.len() == 132 {
                    self.header(ctx);
                    self.buf.clear();
                }
            }
            State::Frames => {
                self.buf.push(s);
                if self.buf.len() == 192 {
                    if crate::symbols::carrier_gone(&self.buf) {
                        if ctx.has_call(KEY) {
                            ctx.end_call(KEY, false);
                        }
                        self.state = State::Idle;
                        self.buf.clear();
                        return SymAnswer::Stop;
                    }
                    self.payload_frame(ctx);
                    self.buf.clear();
                }
            }
        }
        SymAnswer::Continue
    }

    fn on_tick(&mut self, ctx: &mut Ctx) {
        if ctx.has_call(KEY) && ctx.now - ctx.last_frame_s > 1.0 {
            ctx.end_call(KEY, false);
        }
    }

    fn fields(&self) -> Vec<&'static dyn Declare> {
        <DpmrFields as Fields>::SHAPE.fields.to_vec()
    }

    fn publish(&self, r: &mut Published) {
        r.put(&DpmrFields {
            colour_code: self.cc.map(i64::from),
        });
    }

    fn codec(&self) -> Codec {
        Codec::Ambe2
    }
}

// ---------------------------------------------------------------------------------------------------------------------------
// Generator.

fn sync_bits(hex: &str) -> Vec<u8> {
    let mut b = Vec::new();
    push_u64(
        &mut b,
        u64::from_str_radix(hex, 16).unwrap_or(0),
        hex.len() as u32 * 4,
    );
    b
}

/// The bits of one transmission of header `h` and colour code `cc`: `frames` voice frames, whose voice is the test speech
/// from frame `next_speech` on, or the blocks of `data`.
fn transmission(
    h: HeaderInfo,
    cc: u8,
    packet: bool,
    frames: usize,
    data: &[u8],
    next_speech: &mut usize,
) -> Vec<u8> {
    let mut b = Vec::new();
    for _ in 0..36 {
        b.extend([0, 1, 1, 1]); // preamble: +3, -3 alternating
    }
    b.extend(sync_bits(if packet { FS4 } else { FS1 }));
    let hi = encode_block(&h.bits());
    b.extend(&hi);
    push_u64(&mut b, cc_word(cc) as u64, 24);
    b.extend(&hi);
    let voice = h.comm_mode == 0 || h.comm_mode == 4;
    let chunks: Vec<Vec<u8>> = if voice {
        vec![]
    } else {
        let mut d = vec![data.len() as u8];
        d.extend_from_slice(data);
        d.chunks(24)
            .map(|c| {
                let mut v = c.to_vec();
                v.resize(24, 0);
                v
            })
            .collect()
    };
    let n = if voice { frames } else { chunks.len() };
    for f in 0..n {
        if f % 2 == 0 {
            b.extend(sync_bits(FS2));
        } else {
            push_u64(&mut b, cc_word(cc) as u64, 24);
        }
        let mut cch = Vec::with_capacity(48);
        push_u64(&mut cch, (f % 4) as u64, 2);
        push_u64(
            &mut cch,
            ((h.called >> if f % 2 == 0 { 12 } else { 0 }) & 0xFFF) as u64,
            12,
        );
        push_u64(&mut cch, h.comm_mode as u64, 3);
        push_u64(&mut cch, 0, 23);
        let crc = CRC8_SMBUS.bits(&cch);
        push_u64(&mut cch, crc as u64, 8);
        b.extend(encode_block(&cch));
        if voice {
            let speech = crate::voice::test_speech_half();
            for _ in 0..4 {
                b.extend_from_slice(&speech[*next_speech % speech.len()]);
                *next_speech += 1;
            }
        } else {
            b.extend(encode_block(&bytes_to_bits(&chunks[f])));
        }
    }
    b.extend(sync_bits(FS3));
    b.extend([0u8; 48]);
    b
}

/// The dPMR test signal: a group voice call, a status, a short message, packet data to all, an emergency individual call.
/// The voice frames carry the test speech ([`crate::voice::test_speech_half`]).
pub fn render() -> Vec<Complex32> {
    let mut next_speech = 0;
    let mut m = FskModulator::new(2400.0, TxPulse::RootRaisedCosine(0.2), 350.0);
    let mut out = Vec::new();
    silence(0.3, &mut out);
    let txs: Vec<(HeaderInfo, u8, bool, usize, Vec<u8>)> = vec![
        (
            HeaderInfo {
                called: 200,
                own: 1001,
                comm_mode: 0,
                format: 1,
                emergency: false,
            },
            7,
            false,
            24,
            vec![],
        ),
        (
            HeaderInfo {
                called: 1001,
                own: 1002,
                comm_mode: 1,
                format: 0,
                emergency: false,
            },
            7,
            false,
            0,
            vec![17],
        ),
        (
            HeaderInfo {
                called: 2001,
                own: 2005,
                comm_mode: 2,
                format: 0,
                emergency: false,
            },
            12,
            false,
            0,
            b"TEST MESSAGE ON CHANNEL ONE".to_vec(),
        ),
        (
            HeaderInfo {
                called: 0xFF_FFFF,
                own: 2006,
                comm_mode: 5,
                format: 2,
                emergency: false,
            },
            12,
            true,
            0,
            b"PACKET DATA".to_vec(),
        ),
        (
            HeaderInfo {
                called: 2002,
                own: 2001,
                comm_mode: 0,
                format: 0,
                emergency: true,
            },
            12,
            false,
            20,
            vec![],
        ),
    ];
    for (h, cc, packet, frames, data) in txs {
        let bits = transmission(h, cc, packet, frames, &data, &mut next_speech);
        m.symbols(&bits_to_levels(&bits), &mut out);
        m.flush(8, &mut out);
        m.reset();
        silence(0.6, &mut out);
    }
    out
}

/// The dPMR generator from the settings of the stack.
pub fn generator(cfg: &GenConfig) -> Prepared {
    Prepared::new(render(), cfg.rate_hz, true).with_info(6_250.0, 2400.0, 4800.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_and_blocks_round_trip() {
        let h = HeaderInfo {
            called: 200,
            own: 1001,
            comm_mode: 0,
            format: 1,
            emergency: true,
        };
        let mut b = encode_block(&h.bits());
        assert_eq!(b.len(), 120);
        b[17] ^= 1;
        let (info, fixed, ok) = decode_block(&b);
        assert!(ok);
        assert_eq!(fixed, 1);
        assert_eq!(HeaderInfo::from_bits(&info), Some(h));
        for cc in 0..64 {
            assert_eq!(cc_of(cc_word(cc)), Some(cc));
        }
    }
}
