//! NXDN: NXDN48 (2400 symbols/s) and NXDN96 (4800 symbols/s) frames, the scrambler, the link information channel, SACCH,
//! FACCH1, CAC and UDCH/FACCH2 with their codes, the layer 3 messages of calls, status, short data and site information, and
//! the AMBE+2 voice frames.
//! Standard: NXDN TS 1-A to 1-D.

use crate::bits::{bits_to_bytes, bytes_to_bits};
use crate::conv::ConvChannel;
use crate::decoder::{Ctx, ModeParams, Protocol};
use crate::gen::{silence, FskModulator, Prepared, TxPulse};
use crate::items::*;
use crate::symbols::{Sym, SymAnswer, SyncAnswer, SyncPattern};
use crate::typed::NxdnFields;
use sdrkit_algo_fec::conv::{codes::NXDN_K5, Puncture};
use sdrkit_algo_fec::voice::{
    NXDN_CAC_PUNCTURE, NXDN_FACCH1_PUNCTURE, NXDN_SACCH_PUNCTURE, NXDN_UDCH_PUNCTURE,
};
use sdrkit_core::interface::Fields;
use sdrkit_core::media::Codec;
use sdrkit_core::publish::Published;
use sdrkit_core::publish::Declare;
use sdrkit_core::{Complex32, GenConfig, Value};
use sdrkit_util_bits::crc::catalog::{Engine, CRC12_NXDN, CRC15_NXDN, CRC16_IBM_3740, CRC6_NXDN};
use sdrkit_util_bits::interleave::{Permutation, ReadOrder};
use sdrkit_util_bits::pack::{push_u64, to_u64};
use sdrkit_util_bits::seq;

/// The frame sync word.
pub const FSW: &str = "CDF59";

/// Symbols after the frame sync that the scrambler covers.
pub const SCRAMBLED: usize = 182;

/// The scrambler: a sign flip of every symbol after the sync where the NXDN PN9 sequence is one (NXDN TS 1-A).
pub fn scrambler() -> [u8; SCRAMBLED] {
    let mut pn = [0u8; SCRAMBLED];
    let mut l = seq::NXDN;
    l.fill_bits(&mut pn);
    pn
}

/// A coded channel: information bits, CRC, interleave and puncturing (NXDN TS 1-A).
#[derive(Clone, Copy, Debug)]
pub struct Channel {
    pub info: usize,
    pub crc: Engine,
    pub rows: usize,
    pub cols: usize,
    pub puncture: Puncture<'static>,
}

pub const SACCH: Channel = Channel {
    info: 26,
    crc: Engine::U8(&CRC6_NXDN),
    rows: 12,
    cols: 5,
    puncture: NXDN_SACCH_PUNCTURE,
};
pub const FACCH1: Channel = Channel {
    info: 80,
    crc: Engine::U16(&CRC12_NXDN),
    rows: 16,
    cols: 9,
    puncture: NXDN_FACCH1_PUNCTURE,
};
pub const CAC: Channel = Channel {
    info: 155,
    crc: Engine::U16(&CRC16_IBM_3740),
    rows: 12,
    cols: 25,
    puncture: NXDN_CAC_PUNCTURE,
};
pub const UDCH: Channel = Channel {
    info: 184,
    crc: Engine::U16(&CRC15_NXDN),
    rows: 12,
    cols: 29,
    puncture: NXDN_UDCH_PUNCTURE,
};

impl Channel {
    fn crc_bits(&self) -> usize {
        self.crc.spec().width as usize
    }
    /// Input bits of the code: information, CRC and the 4 tail bits.
    fn steps(&self) -> usize {
        self.info + self.crc_bits() + 4
    }
    pub fn coded_len(&self) -> usize {
        self.rows * self.cols
    }
}

/// The coder of one channel: its convolutional code and interleave, with the buffers of a frame.
#[derive(Clone, Debug)]
pub struct ChannelCoder {
    ch: Channel,
    conv: ConvChannel,
    /// The interleave: the coded bits are written in rows of `rows` and read by columns.
    order: Permutation,
    soft: Vec<f32>,
}

impl ChannelCoder {
    pub fn new(ch: Channel) -> ChannelCoder {
        ChannelCoder {
            ch,
            conv: ConvChannel::new(NXDN_K5, Some(ch.puncture), ch.steps()),
            order: Permutation::row_column(ch.cols, ch.rows, ReadOrder::Columns)
                .expect("a whole matrix"),
            soft: vec![0.0; ch.coded_len()],
        }
    }

    /// Encodes the information bits (the first `info` of `info`) into the bits sent.
    pub fn encode(&self, info: &[u8]) -> Vec<u8> {
        let mut b = info[..self.ch.info].to_vec();
        let crc = self.ch.crc.bits(&b);
        push_u64(&mut b, crc, self.ch.crc_bits() as u32);
        b.extend([0, 0, 0, 0]);
        let mut coded = Vec::with_capacity(self.ch.coded_len());
        self.conv.encode(&b, &mut coded);
        let mut out = vec![0u8; self.ch.coded_len()];
        if self.order.apply(&coded, &mut out).is_err() {
            out.clear();
        }
        out
    }

    /// Decodes soft values of the bits sent: the information, whether the CRC checks, and the bits repaired.
    pub fn decode(&mut self, soft: &[f32]) -> (Vec<u8>, bool, usize) {
        let n = self.ch.coded_len();
        if soft.len() < n || self.order.invert(&soft[..n], &mut self.soft).is_err() {
            return (vec![0; self.ch.info], false, 0);
        }
        match self.conv.decode(&self.soft, self.ch.steps(), true) {
            Ok((bits, errs)) => {
                let ok = self
                    .ch
                    .crc
                    .verify_bits(&bits[..self.ch.info + self.ch.crc_bits()]);
                (bits[..self.ch.info].to_vec(), ok, errs)
            }
            Err(_) => (vec![0; self.ch.info], false, 0),
        }
    }
}

/// The coders of the four channels.
#[derive(Clone, Debug)]
pub struct Coders {
    pub sacch: ChannelCoder,
    pub facch1: ChannelCoder,
    pub cac: ChannelCoder,
    pub udch: ChannelCoder,
}

impl Default for Coders {
    fn default() -> Self {
        Coders {
            sacch: ChannelCoder::new(SACCH),
            facch1: ChannelCoder::new(FACCH1),
            cac: ChannelCoder::new(CAC),
            udch: ChannelCoder::new(UDCH),
        }
    }
}

/// The link information channel.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Lich {
    /// 0 control (RCCH), 1 traffic (RTCH), 2 data (RDCH), 3 composite.
    pub rf: u8,
    pub fct: u8,
    pub opt: u8,
    /// Outbound (from the repeater or base).
    pub outbound: bool,
}

impl Lich {
    pub fn bits(&self) -> [u8; 8] {
        let v = (self.rf << 6) | (self.fct << 4) | (self.opt << 2) | ((self.outbound as u8) << 1);
        let p = ((v >> 7) ^ (v >> 6) ^ (v >> 5) ^ (v >> 4)) & 1;
        let v = v | p;
        std::array::from_fn(|i| (v >> (7 - i)) & 1)
    }
    pub fn decode(b: &[u8]) -> Option<Lich> {
        let v = b.iter().take(8).fold(0u8, |a, &x| (a << 1) | x);
        let p = ((v >> 7) ^ (v >> 6) ^ (v >> 5) ^ (v >> 4)) & 1;
        (p == v & 1).then_some(Lich {
            rf: v >> 6,
            fct: (v >> 4) & 3,
            opt: (v >> 2) & 3,
            outbound: v & 2 != 0,
        })
    }
}

/// Call types of layer 3.
pub fn call_type_name(t: u8) -> &'static str {
    match t {
        0 => "broadcast",
        1 => "group",
        2 => "unspecified",
        4 => "individual",
        6 => "interconnect",
        7 => "speed dial",
        _ => "reserved",
    }
}

#[derive(Clone, Debug, Default)]
struct Lane {
    buf: Vec<Sym>,
    active: bool,
    idle: usize,
    sacch: [Option<[u8; 18]>; 4],
    data: Option<DataCall>,
}

#[derive(Clone, Debug, Default)]
struct DataCall {
    src: u16,
    dst: u16,
    call_type: u8,
    blocks: usize,
    bytes: Vec<u8>,
    started: f64,
    seen: Vec<usize>,
}

/// The NXDN protocol.
pub struct Nxdn {
    lanes: [Lane; 2],
    ran: Option<u8>,
    site: SiteRecord,
    frames: u64,
    crc_ok: u64,
    crc_bad: u64,
    recent_status: Vec<(u16, u16, f64)>,
    coders: Coders,
    pn: [u8; SCRAMBLED],
}

impl Default for Nxdn {
    fn default() -> Self {
        Nxdn {
            lanes: Default::default(),
            ran: None,
            site: SiteRecord::default(),
            frames: 0,
            crc_ok: 0,
            crc_bad: 0,
            recent_status: Vec::new(),
            coders: Coders::default(),
            pn: scrambler(),
        }
    }
}

const KEY: u32 = 1;

impl Nxdn {
    fn baud(ctx: &Ctx) -> u32 {
        if ctx.lane == 0 {
            4800
        } else {
            2400
        }
    }

    fn crc(&mut self, ok: bool, bits: usize, fixed: usize, ctx: &mut Ctx) {
        if ok {
            self.crc_ok += 1;
        } else {
            self.crc_bad += 1;
        }
        ctx.block(ok, bits, fixed);
    }

    fn frame(&mut self, lane: usize, ctx: &mut Ctx) {
        let buf = std::mem::take(&mut self.lanes[lane].buf);
        if crate::symbols::carrier_gone(&buf[10..]) {
            self.lanes[lane].active = false;
            self.lanes[lane].buf = buf;
            self.lanes[lane].buf.clear();
            return;
        }
        let pn = self.pn;
        // Descrambled soft dibits.
        let mut soft = Vec::with_capacity(364);
        let mut hard = Vec::with_capacity(364);
        for (k, s) in buf[10..192].iter().enumerate() {
            let mut s = *s;
            if pn[k] != 0 {
                s.v = -s.v;
                s.d = -s.d;
            }
            let (a, b) = s.soft_dibit();
            soft.push(a);
            soft.push(b);
            let d = s.dibit();
            hard.push(d >> 1);
            hard.push(d & 1);
        }
        let lich_bits: Vec<u8> = (0..8).map(|i| hard[2 * i]).collect();
        let lich = Lich::decode(&lich_bits);
        ctx.identity(lich.is_some());
        self.lanes[lane].buf = buf;
        self.lanes[lane].buf.clear();
        let Some(lich) = lich else {
            return;
        };
        self.frames += 1;
        let body = &soft[16..];
        match lich.rf {
            0 => {
                // Control channel: CAC outbound (or the inbound short CAC, not decoded).
                if lich.outbound {
                    let (info, ok, errs) = self.coders.cac.decode(&body[..300]);
                    self.crc(ok, 300, errs, ctx);
                    if ok {
                        let ran = (to_u64(&info[2..8])) as u8;
                        self.ran = Some(ran);
                        let msg = bits_to_bytes(&info[8..152]);
                        self.message(&msg, ran, lane, ctx);
                    }
                }
            }
            1 | 3 => {
                // Traffic: SACCH, then two halves of voice or FACCH1.
                let (info, ok, errs) = self.coders.sacch.decode(&body[..60]);
                self.crc(ok, 60, errs, ctx);
                let mut ran = self.ran.unwrap_or(0);
                if ok {
                    let sr = to_u64(&info[..2]) as usize;
                    ran = to_u64(&info[2..8]) as u8;
                    self.ran = Some(ran);
                    let mut frag = [0u8; 18];
                    frag.copy_from_slice(&info[8..26]);
                    // Superframe: SR 3, 2, 1, 0.
                    if lich.fct == 2 {
                        let idx = 3 - sr;
                        if idx == 0 {
                            self.lanes[lane].sacch = Default::default();
                        }
                        self.lanes[lane].sacch[idx] = Some(frag);
                        if idx == 3 && self.lanes[lane].sacch.iter().all(|f| f.is_some()) {
                            let bits: Vec<u8> = self.lanes[lane]
                                .sacch
                                .iter()
                                .flatten()
                                .flatten()
                                .copied()
                                .collect();
                            let msg = bits_to_bytes(&bits);
                            if !ctx.has_call(KEY) {
                                self.message(&msg, ran, lane, ctx);
                            }
                        }
                    }
                }
                for half in 0..2 {
                    let voice = (lich.opt >> (1 - half)) & 1 != 0;
                    let part = &body[60 + half * 144..60 + (half + 1) * 144];
                    if voice {
                        if ctx.has_call(KEY) {
                            ctx.update_call(KEY, |c| c.voice_frames += 2);
                            for k in 0..2 {
                                let b: Vec<u8> = part[k * 72..k * 72 + 72]
                                    .iter()
                                    .map(|&v| (v > 0.0) as u8)
                                    .collect();
                                let cw = sdrkit_codec_mbe::codewords(&b);
                                let t = ctx.now + (half * 2 + k) as f64 * 0.02;
                                let spec = crate::voice::FrameSpec {
                                    codec: Codec::Ambe2,
                                    packaging: sdrkit_graph::SpeechPackaging::Codewords,
                                    codec_mode: None,
                                    duration_ms: 20,
                                    stack: "nxdn",
                                    carrier_hz: ctx.carrier_hz,
                                    slot: 0,
                                };
                                ctx.speech(KEY, spec, &cw, t, false, None);
                            }
                        }
                    } else {
                        let (info, ok, errs) = self.coders.facch1.decode(part);
                        self.crc(ok, 144, errs, ctx);
                        if ok {
                            let msg = bits_to_bytes(&info);
                            self.message(&msg, ran, lane, ctx);
                        }
                    }
                }
            }
            2 => {
                // Data: UDCH or FACCH2.
                let (info, ok, errs) = self.coders.udch.decode(&body[..348]);
                self.crc(ok, 348, errs, ctx);
                if ok {
                    let msg = bits_to_bytes(&info);
                    let ran = self.ran.unwrap_or(0);
                    self.message(&msg, ran, lane, ctx);
                }
            }
            _ => {}
        }
    }

    fn record(
        &self,
        kind: CallKind,
        src: u16,
        dst: u16,
        call_type: u8,
        ran: u8,
        ctx: &Ctx,
    ) -> CallRecord {
        let mut rec = CallRecord {
            kind,
            src: Ident::Number(src as u64),
            dst: Ident::Number(dst as u64),
            group: call_type == 1 || call_type == 0,
            access: Some(Access::new(AccessKind::Ran, u32::from(ran))),
            encryption: Encryption::clear(),
            ..Default::default()
        };
        rec.set(CallDetail::Baud, Value::Hz(f64::from(Self::baud(ctx))));
        rec.set(
            CallDetail::CallType,
            Value::Text(call_type_name(call_type).into()),
        );
        rec
    }

    /// A layer 3 message (octet 0: two flags and the message type).
    fn message(&mut self, m: &[u8], ran: u8, lane: usize, ctx: &mut Ctx) {
        let mt = m[0] & 0x3F;
        let text = |name: &str| format!("{name} ({mt:02X})");
        let src = u16::from_be_bytes([m[2], m[3]]);
        let dst = u16::from_be_bytes([m[4], m[5]]);
        let call_type = m[1] >> 5;
        match mt {
            // VCALL: octet 1 call type and options, source, destination, cipher and key.
            0x01 => {
                let cipher = m[6] >> 6;
                let key = m[6] & 0x3F;
                if let Some(c) = ctx.call(KEY) {
                    if c.src == Ident::Number(src as u64) && c.dst == Ident::Number(dst as u64) {
                        return;
                    }
                }
                let mut rec = self.record(CallKind::Voice, src, dst, call_type, ran, ctx);
                rec.emergency = m[1] & 0x10 != 0;
                let enc = if cipher == 0 {
                    Encryption::clear()
                } else {
                    Encryption::encrypted(
                        Some(cipher as u32),
                        match cipher {
                            1 => "scrambler",
                            2 => "DES",
                            _ => "AES",
                        },
                        Some(key as u32),
                    )
                };
                rec.encryption = enc.clone();
                ctx.start_call(KEY, rec, false);
                ctx.set_encryption(KEY, enc);
                ctx.control(ControlRecord {
                    opcode: mt as u32,
                    name: "VCALL".into(),
                    text: format!("voice call {src} -> {dst} ({})", call_type_name(call_type)),
                    time: ctx.now,
                    ..Default::default()
                });
            }
            // TX_REL: the end of a transmission.
            0x08 => {
                if ctx.has_call(KEY) {
                    ctx.end_call(KEY, true);
                }
                if let Some(d) = self.lanes[lane].data.take() {
                    self.finish_data(d, ran, ctx);
                }
            }
            // DCALL_HDR: a data call's header (octet 6: the number of data blocks).
            0x38 => {
                let blocks = (m[6] as usize).max(1);
                if self.lanes[lane]
                    .data
                    .as_ref()
                    .is_some_and(|d| d.src == src && d.dst == dst)
                {
                    return;
                }
                self.lanes[lane].data = Some(DataCall {
                    src,
                    dst,
                    call_type,
                    blocks,
                    bytes: Vec::new(),
                    started: ctx.now,
                    seen: Vec::new(),
                });
            }
            // DCALL_UDT: a data block (octet 1: its number).
            0x39 => {
                let finish = if let Some(d) = self.lanes[lane].data.as_mut() {
                    let n = m[1] as usize;
                    if !d.seen.contains(&n) {
                        d.seen.push(n);
                        d.bytes.resize((n + 1) * (m.len() - 2), 0);
                        d.bytes[n * (m.len() - 2)..(n + 1) * (m.len() - 2)]
                            .copy_from_slice(&m[2..]);
                    }
                    d.seen.len() >= d.blocks
                } else {
                    false
                };
                if finish {
                    if let Some(d) = self.lanes[lane].data.take() {
                        self.finish_data(d, ran, ctx);
                    }
                }
            }
            // A status message: octet 6 is the status.
            0x32 => {
                let status = m[6];
                let now = ctx.now;
                self.recent_status.retain(|r| now - r.2 < 2.0);
                if self.recent_status.iter().any(|r| r.0 == src && r.1 == dst) {
                    return;
                }
                self.recent_status.push((src, dst, now));
                let mut rec = self.record(CallKind::Status, src, dst, call_type, ran, ctx);
                rec.text = format!("status {status}");
                rec.set(CallDetail::Status, Value::I64(i64::from(status)));
                ctx.log_single(rec);
            }
            // SITE_INFO.
            0x18 => {
                let loc = u32::from_be_bytes([0, m[1], m[2], m[3]]);
                self.site.location = Some(loc);
                self.site.system = Some((loc >> 10) & 0xFFF);
                self.site.site = Some(loc & 0x3FF);
                self.site.ran = Some(u32::from(ran));
                self.site.valid = true;
                self.site.time = ctx.now;
                ctx.site(self.site.clone());
                ctx.control(ControlRecord {
                    opcode: mt as u32,
                    name: "SITE_INFO".into(),
                    text: format!("site information: location {loc:06X}"),
                    time: ctx.now,
                    ..Default::default()
                });
            }
            0x10 => {}
            0x19..=0x1B => {
                let name = match mt {
                    0x19 => "SRV_INFO",
                    0x1A => "CCH_INFO",
                    _ => "ADJ_SITE_INFO",
                };
                ctx.control(ControlRecord {
                    opcode: mt as u32,
                    name: name.into(),
                    text: name.to_string(),
                    time: ctx.now,
                    ..Default::default()
                });
            }
            _ => {
                ctx.control(ControlRecord {
                    opcode: mt as u32,
                    name: "message".into(),
                    text: text("layer 3 message"),
                    time: ctx.now,
                    ..Default::default()
                });
            }
        }
    }

    fn finish_data(&mut self, d: DataCall, ran: u8, ctx: &mut Ctx) {
        let text: String = d
            .bytes
            .iter()
            .take_while(|&&b| b != 0)
            .map(|&b| b as char)
            .collect();
        let mut rec = self.record(CallKind::ShortData, d.src, d.dst, d.call_type, ran, ctx);
        rec.text = text.trim_end().to_string();
        let now = ctx.now;
        ctx.now = d.started;
        ctx.log_single(rec.clone());
        ctx.message(MessageRecord {
            src: rec.src.clone(),
            dst: rec.dst.clone(),
            group: rec.group,
            format: MessageFormat::ShortData,
            data_format: None,
            text: rec.text.clone(),
            crc_ok: true,
            time: d.started,
            slot: 0,
            sms: false,
        });
        ctx.now = now;
    }
}

impl Protocol for Nxdn {
    fn params(&self) -> ModeParams {
        ModeParams {
            mode: "nxdn",
            name: "NXDN",
            baud: 4800.0,
            two_level: false,
            channel_hz: 12_500.0,
            calls: &crate::results::CALLS_300,
            message_cap: 100,
            polarity_rule: false,
            clock_loop: true,
            extra_baud: Some(2400.0),
            stream_rate: crate::front::RATE,
        }
    }

    fn patterns(&self) -> Vec<SyncPattern> {
        vec![SyncPattern::from_hex_dibits(0, FSW, 0).with_max_err(1)]
    }

    fn on_sync(
        &mut self,
        _id: u8,
        _inv: bool,
        pre: &[Sym],
        _at: u64,
        tracking: bool,
        ctx: &mut Ctx,
    ) -> SyncAnswer {
        let lane = ctx.lane as usize;
        let l = &mut self.lanes[lane];
        // Within a frame the sync word can turn up in the payload: a sync is taken at a frame boundary or when not in step.
        if tracking && l.active && l.buf.len() > 12 && l.buf.len() < 186 {
            return SyncAnswer::Ignore;
        }
        l.buf.clear();
        l.buf.extend_from_slice(pre);
        l.active = true;
        l.idle = 0;
        SyncAnswer::Take
    }

    fn on_symbol(&mut self, s: Sym, _at: u64, ctx: &mut Ctx) -> SymAnswer {
        let lane = ctx.lane as usize;
        if !self.lanes[lane].active {
            self.lanes[lane].idle += 1;
            return if self.lanes[lane].idle > 400 {
                SymAnswer::Stop
            } else {
                SymAnswer::Continue
            };
        }
        self.lanes[lane].buf.push(s);
        if self.lanes[lane].buf.len() == 192 {
            self.frame(lane, ctx);
            self.lanes[lane].active = false;
        }
        SymAnswer::Continue
    }

    fn on_tick(&mut self, ctx: &mut Ctx) {
        if ctx.has_call(KEY) && ctx.now - ctx.last_frame_s > 1.0 {
            ctx.end_call(KEY, false);
        }
    }

    fn fields(&self) -> Vec<&'static dyn Declare> {
        <NxdnFields as Fields>::SHAPE.fields.to_vec()
    }

    fn publish(&self, r: &mut Published) {
        r.put(&NxdnFields {
            ran: self.ran.map(i64::from),
            nxdn_frames: self.frames as i64,
            crc_ok: self.crc_ok as i64,
            crc_bad: self.crc_bad as i64,
        });
    }

    fn codec(&self) -> Codec {
        Codec::Ambe2
    }
}

// ---------------------------------------------------------------------------------------------------------------------------
// Generator.

fn frame_bits(lich: Lich, payload: &[u8]) -> Vec<u8> {
    let mut b = Vec::with_capacity(384);
    push_u64(&mut b, u64::from_str_radix(FSW, 16).unwrap_or(0), 20);
    let mut body = Vec::with_capacity(364);
    // Each LICH bit is the sign of an outer symbol (dibit b, 1).
    for &x in &lich.bits() {
        body.push(x);
        body.push(1);
    }
    body.extend_from_slice(payload);
    body.resize(364, 0);
    // Scramble: flip the sign bit of every dibit where PN9 is one.
    let pn = scrambler();
    for k in 0..SCRAMBLED {
        body[2 * k] ^= pn[k];
    }
    b.extend(body);
    b
}

fn l3_call(mt: u8, call_type: u8, src: u16, dst: u16, extra: &[u8]) -> Vec<u8> {
    let mut m = vec![mt, call_type << 5];
    m.extend_from_slice(&src.to_be_bytes());
    m.extend_from_slice(&dst.to_be_bytes());
    m.extend_from_slice(extra);
    m
}

fn pad_bits(m: &[u8], n: usize) -> Vec<u8> {
    let mut b = bytes_to_bits(m);
    b.resize(n, 0);
    b
}

struct Gen {
    coders: Coders,
    /// The next frame of the test speech.
    next_speech: usize,
}

impl Gen {
    /// A traffic frame: the SACCH, and in each half a FACCH1 of `info` or, for `None`, the next two frames of the test
    /// speech.
    fn traffic_frame(&mut self, lich: Lich, sacch: &[u8], halves: [Option<Vec<u8>>; 2]) -> Vec<u8> {
        let mut payload = self.coders.sacch.encode(sacch);
        let speech = crate::voice::test_speech_half();
        for h in halves.iter() {
            match h {
                Some(info) => payload.extend(self.coders.facch1.encode(info)),
                None => {
                    for _ in 0..2 {
                        payload.extend_from_slice(&speech[self.next_speech % speech.len()]);
                        self.next_speech += 1;
                    }
                }
            }
        }
        frame_bits(lich, &payload)
    }
}

fn sacch_info(sr: u8, ran: u8, frag: &[u8]) -> Vec<u8> {
    let mut b = Vec::with_capacity(26);
    push_u64(&mut b, sr as u64, 2);
    push_u64(&mut b, ran as u64, 6);
    b.extend_from_slice(&frag[..18]);
    b
}

/// A voice call: header frame (VCALL in FACCH1 twice), voice frames with the call in the SACCH superframe, TX_REL.
fn voice_call(
    g: &mut Gen,
    ran: u8,
    call_type: u8,
    src: u16,
    dst: u16,
    frames: usize,
) -> Vec<u8> {
    let vcall = pad_bits(&l3_call(0x01, call_type, src, dst, &[0, 0]), 80);
    let sf = pad_bits(&l3_call(0x01, call_type, src, dst, &[0, 0]), 72);
    let mut bits = Vec::new();
    let hdr = Lich {
        rf: 1,
        fct: 0,
        opt: 0,
        outbound: true,
    };
    bits.extend(g.traffic_frame(
        hdr,
        &sacch_info(0, ran, &[0; 18]),
        [Some(vcall.clone()), Some(vcall.clone())],
    ));
    let voice = Lich {
        rf: 1,
        fct: 2,
        opt: 3,
        outbound: true,
    };
    for f in 0..frames {
        let idx = f % 4;
        let si = sacch_info(3 - idx as u8, ran, &sf[idx * 18..idx * 18 + 18]);
        bits.extend(g.traffic_frame(voice, &si, [None, None]));
    }
    let rel = pad_bits(&l3_call(0x08, call_type, src, dst, &[]), 80);
    bits.extend(g.traffic_frame(
        hdr,
        &sacch_info(0, ran, &[0; 18]),
        [Some(rel.clone()), Some(rel)],
    ));
    bits
}

/// The NXDN test signal: a trunked site's control channel (NXDN96), an NXDN48 group call, a status, an NXDN96 short data call
/// and an NXDN96 individual call. The voice frames carry the test speech ([`crate::voice::test_speech_half`]).
pub fn render() -> Vec<Complex32> {
    let mut g = Gen {
        coders: Coders::default(),
        next_speech: 0,
    };
    let mut out = Vec::new();
    silence(0.3, &mut out);
    let mut m96 = FskModulator::new(4800.0, TxPulse::RaisedCosine(0.2), 800.0);
    let mut m48 = FskModulator::new(2400.0, TxPulse::RaisedCosine(0.2), 350.0);
    let send = |m: &mut FskModulator, bits: &[u8], out: &mut Vec<Complex32>, gap: f64| {
        m.symbols(&crate::gen::bits_to_levels(bits), out);
        m.flush(8, out);
        m.reset();
        silence(gap, out);
    };
    // Control channel: SITE_INFO, RAN 5, location 0A4C2B.
    let mut cc = Vec::new();
    for _ in 0..25 {
        let mut info = Vec::with_capacity(155);
        push_u64(&mut info, 0, 2);
        push_u64(&mut info, 5, 6);
        let mut msg = vec![0x18, 0x0A, 0x4C, 0x2B];
        msg.resize(18, 0);
        info.extend(bytes_to_bits(&msg));
        info.resize(155, 0);
        let mut payload = g.coders.cac.encode(&info);
        payload.resize(348, 0);
        cc.extend(frame_bits(
            Lich {
                rf: 0,
                fct: 0,
                opt: 0,
                outbound: true,
            },
            &payload,
        ));
    }
    send(&mut m96, &cc, &mut out, 0.6);
    let call = voice_call(&mut g, 1, 1, 1001, 200, 24);
    send(&mut m48, &call, &mut out, 0.6);
    // A status 17 from 1002 to 1001.
    let st = pad_bits(&l3_call(0x32, 4, 1002, 1001, &[17]), 80);
    let hdr = Lich {
        rf: 1,
        fct: 0,
        opt: 0,
        outbound: true,
    };
    let f = g.traffic_frame(
        hdr,
        &sacch_info(0, 1, &[0; 18]),
        [Some(st.clone()), Some(st)],
    );
    send(&mut m48, &f, &mut out, 0.6);
    // Short data 2005 -> 2001: header and two data blocks in FACCH1, then the release.
    {
        let hdr = Lich {
            rf: 1,
            fct: 0,
            opt: 0,
            outbound: true,
        };
        let text = b"TEST MESSAGE 96\0";
        let dh = pad_bits(&l3_call(0x38, 4, 2005, 2001, &[2]), 80);
        let mut b0 = vec![0x39, 0];
        b0.extend_from_slice(&text[..8]);
        let mut b1 = vec![0x39, 1];
        b1.extend_from_slice(&text[8..]);
        let rel = pad_bits(&l3_call(0x08, 4, 2005, 2001, &[]), 80);
        let mut bits = g.traffic_frame(
            hdr,
            &sacch_info(0, 5, &[0; 18]),
            [Some(dh), Some(pad_bits(&b0, 80))],
        );
        bits.extend(g.traffic_frame(
            hdr,
            &sacch_info(0, 5, &[0; 18]),
            [Some(pad_bits(&b1, 80)), Some(rel)],
        ));
        send(&mut m96, &bits, &mut out, 0.6);
    }
    let call = voice_call(&mut g, 5, 4, 2001, 2002, 36);
    send(&mut m96, &call, &mut out, 0.6);
    out
}

/// The NXDN generator from the settings of the stack.
pub fn generator(cfg: &GenConfig) -> Prepared {
    Prepared::new(render(), cfg.rate_hz, true).with_info(12_500.0, 4800.0, 9600.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn channels_round_trip() {
        for ch in [SACCH, FACCH1, CAC, UDCH] {
            let mut ch_coder = ChannelCoder::new(ch);
            let info: Vec<u8> = (0..ch.info).map(|i| ((i * 7 + 1) % 3 == 0) as u8).collect();
            let sent = ch_coder.encode(&info);
            assert_eq!(sent.len(), ch.coded_len());
            let mut soft: Vec<f32> = sent
                .iter()
                .map(|&b| if b != 0 { 1.0 } else { -1.0 })
                .collect();
            soft[7] = -soft[7];
            let (got, ok, _) = ch_coder.decode(&soft);
            assert!(ok);
            assert_eq!(got, info);
        }
    }

    #[test]
    fn lich_parity() {
        let l = Lich {
            rf: 1,
            fct: 2,
            opt: 3,
            outbound: true,
        };
        assert_eq!(Lich::decode(&l.bits()), Some(l));
        assert_eq!(l.bits(), [0, 1, 1, 0, 1, 1, 1, 0]);
    }
}
