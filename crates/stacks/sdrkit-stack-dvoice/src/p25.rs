//! P25 Phase 1: C4FM frames with status symbols, the network identifier, header, voice (LDU1, LDU2), terminators, trunking
//! signalling blocks, packet data headers, link control, encryption sync, the site and channel tables, grants, and the IMBE
//! voice frames.
//! Standard: TIA-102.BAAA.

use crate::bits::{bits_to_bytes, bytes_to_bits};
use crate::decoder::{Ctx, ModeParams, Protocol};
use crate::gen::{bits_to_levels, silence, FskModulator, Prepared, TxPulse};
use crate::items::*;
use crate::symbols::{Sym, SymAnswer, SyncAnswer, SyncPattern};
use crate::typed::{AdjacentRow, IdentifierRow, P25Fields, ADJACENT, IDENTIFIERS};
use sdrkit_algo_fec::bch::{codes::P25_NID_63_16, Bch};
use sdrkit_algo_fec::conv::tcm::{TcmCodec, P25_1_2};
use sdrkit_algo_fec::rs::{codes as rs_codes, Rs};
use sdrkit_algo_fec::small::SmallCode;
use sdrkit_core::interface::{Fields, Hertz};
use sdrkit_core::media::Codec;
use sdrkit_core::publish::{field, Declare, Published, TableDef};
use sdrkit_core::{Complex32, GenConfig, Value};
use sdrkit_util_bits::crc::catalog::CRC16_GSM;
use sdrkit_util_bits::crc::Trailer;
use sdrkit_util_bits::pack::{push_u64, to_u64};
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::sync::LazyLock;

/// The frame sync.
pub const FS: &str = "5575F5FF77FF";

/// Data unit IDs.
pub mod duid {
    pub const HDU: u8 = 0x0;
    pub const TDU: u8 = 0x3;
    pub const LDU1: u8 = 0x5;
    pub const TSDU: u8 = 0x7;
    pub const LDU2: u8 = 0xA;
    pub const PDU: u8 = 0xC;
    pub const TDULC: u8 = 0xF;
}

/// The name of an algorithm ID.
pub fn alg_name(alg: u8) -> &'static str {
    match alg {
        0x80 => "clear",
        0x81 => "DES-OFB",
        0x83 => "Triple DES",
        0x84 => "AES-256",
        0x85 => "AES-128",
        0xAA => "ADP (RC4)",
        0x9F => "DES-XL",
        _ => "unknown",
    }
}

/// A channel identifier's parameters.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Iden {
    pub id: u8,
    pub vhf_uhf: bool,
    pub base_hz: f64,
    pub spacing_hz: f64,
    pub tx_offset_hz: f64,
    pub bandwidth_hz: f64,
}

/// Interleaves the status symbols: data bits in, frame bits out (a status dibit after every 70 bits, counted from the frame start).
pub fn insert_status(data: &[u8], status: u8) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len() + data.len() / 35 + 2);
    for &b in data {
        out.push(b);
        if out.len() % 72 == 70 {
            out.push(status >> 1);
            out.push(status & 1);
        }
    }
    out
}

/// The Hamming (10,6,3) of the link control and encryption sync hex bits (TIA-102.BAAA): the generator rows of the information
/// bits, the first check bit most significant.
const HAMMING_10_6_ROWS: [u32; 6] = [0b1110, 0b1101, 0b1011, 0b0111, 0b0011, 0b1100];

/// Hamming (10,6,3) of LDU hex bits.
static HEX_WORD: LazyLock<SmallCode> = LazyLock::new(|| {
    SmallCode::from_parity_rows(10, 6, &HAMMING_10_6_ROWS).expect("a valid Hamming code")
});
/// Golay (18,6) of the header hex bits.
static HDU_WORD: LazyLock<SmallCode> = LazyLock::new(SmallCode::golay_18_6);
/// Extended Golay (24,12) of the terminator link control (two hex bits a word).
static TDULC_WORD: LazyLock<SmallCode> = LazyLock::new(SmallCode::golay_24_12);
/// The shortened cyclic (16,8) code of the low speed data.
static LSD_WORD: LazyLock<SmallCode> =
    LazyLock::new(|| SmallCode::cyclic(16, 0x13B).expect("a valid cyclic code"));
/// The NID encoder (encoding needs no scratch space).
static NID: LazyLock<Bch> =
    LazyLock::new(|| Bch::from_code(&P25_NID_63_16).expect("a valid BCH code"));

/// The decoders a P25 receiver keeps (they hold scratch space).
#[derive(Clone, Debug)]
pub struct Codecs {
    pub nid: Bch,
    pub rs_24_12: Rs,
    pub rs_24_16: Rs,
    pub rs_36_20: Rs,
    pub tcm: TcmCodec,
}

impl Default for Codecs {
    fn default() -> Self {
        Codecs {
            nid: Bch::from_code(&P25_NID_63_16).expect("a valid BCH code"),
            rs_24_12: Rs::new(rs_codes::P25_24_12).expect("a valid RS code"),
            rs_24_16: Rs::new(rs_codes::P25_24_16).expect("a valid RS code"),
            rs_36_20: Rs::new(rs_codes::P25_36_20).expect("a valid RS code"),
            tcm: TcmCodec::new(P25_1_2).expect("a valid trellis code"),
        }
    }
}

/// Encodes the NID: NAC and DUID with the BCH(63,16) code, then an even parity bit (TIA-102.BAAA).
pub fn encode_nid(nac: u16, duid: u8) -> u64 {
    let info = ((nac as u64 & 0xFFF) << 4) | (duid as u64 & 0xF);
    let mut w = [0u8; 63];
    for (i, b) in w[..16].iter_mut().enumerate() {
        *b = (info >> (15 - i)) as u8 & 1;
    }
    NID.encode(&mut w).expect("63 bits");
    let v = to_u64(&w);
    (v << 1) | (v.count_ones() as u64 & 1)
}

/// Decodes the 64 NID bits: the NAC, the DUID and the bits corrected. `None` when more than 11 bits are wrong (the even parity
/// bit counts as a twelfth error).
pub fn decode_nid(bch: &mut Bch, nid: u64) -> Option<(u16, u8, u32)> {
    let mut w = [0u8; 63];
    for (i, b) in w.iter_mut().enumerate() {
        *b = (nid >> (63 - i)) as u8 & 1;
    }
    let fixed = bch.decode(&mut w).ok()?.corrected;
    let v = to_u64(&w);
    let parity_wrong = ((v.count_ones() as u64 + (nid & 1)) & 1) as u32;
    if fixed + parity_wrong > 11 {
        return None;
    }
    let info = v >> 47;
    Some(((info >> 4) as u16, (info & 0xF) as u8, fixed + parity_wrong))
}

/// Encodes information hex bits with the RS code and each hex bit with Hamming (10,6,3): the 240 bits of an LDU's link control
/// or encryption sync.
pub fn encode_ldu_hexbits(info_hex: &[u8], rs: &Rs) -> Vec<u8> {
    let k = rs.code().k as usize;
    let n = rs.code().n as usize;
    let mut w = vec![0u8; n];
    w[..k].copy_from_slice(&info_hex[..k]);
    rs.encode_word(&mut w).expect("RS sizes");
    let mut out = Vec::with_capacity(240);
    for &h in &w {
        push_u64(&mut out, HEX_WORD.encode(h as u128) as u64, 10);
    }
    out
}

/// Decodes 240 bits of hex bits: Hamming (10,6) per hex bit, then the RS code with the hex bits whose Hamming word did not decode
/// as erasures. Returns the information hex bits, whether the RS code word checks, and the bits repaired.
pub fn decode_ldu_hexbits(bits: &[u8], rs: &mut Rs) -> (Vec<u8>, bool, usize) {
    let n = rs.code().n as usize;
    let k = rs.code().k as usize;
    let mut w = vec![0u8; n];
    let mut erasures: Vec<u16> = Vec::with_capacity(n);
    let mut fixed = 0;
    for (i, c) in bits.chunks(10).take(n).enumerate() {
        match HEX_WORD.decode(to_u64(c) as u128) {
            Some((cw, e)) => {
                fixed += e as usize;
                w[i] = HEX_WORD.info(cw) as u8;
            }
            None => {
                w[i] = to_u64(&c[..6]) as u8;
                erasures.push(i as u16);
            }
        }
    }
    match rs.decode(&mut w, &erasures) {
        Ok(d) => (w[..k].to_vec(), true, fixed + d.corrected as usize * 6),
        Err(_) => (w[..k].to_vec(), false, fixed),
    }
}

fn hex_to_bits(hex: &[u8]) -> Vec<u8> {
    let mut b = Vec::with_capacity(hex.len() * 6);
    for &h in hex {
        push_u64(&mut b, h as u64, 6);
    }
    b
}

fn bits_to_hex(bits: &[u8]) -> Vec<u8> {
    bits.chunks(6).map(|c| to_u64(c) as u8).collect()
}

/// The positions of the parts of an LDU in its 1680 data bits (status removed): voice frames, LC/ES hex bits, low speed data.
pub const LDU_VOICE: [usize; 9] = [112, 256, 440, 624, 808, 992, 1176, 1360, 1536];
pub const LDU_LC: [usize; 6] = [400, 584, 768, 952, 1136, 1320];
pub const LDU_LSD: usize = 1504;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Want {
    Nid,
    Body(u8, usize),
    Idle,
}

/// The P25 Phase 1 protocol.
pub struct P25 {
    want: Want,
    k: usize,
    bits: Vec<u8>,
    nac: Option<u16>,
    idle_syms: usize,
    idens: BTreeMap<u8, Iden>,
    site: SiteRecord,
    adjacent: BTreeMap<u32, Adjacent>,
    // The call in progress.
    call_lc: Option<(bool, u32, u32, bool)>,
    enc: Option<(u8, u16)>,
    last_end: Option<(u32, f64)>,
    pdu_blocks: usize,
    frames: u64,
    header_at: Option<f64>,
    recent_grants: Vec<(u32, u16, u32, f64)>,
    /// The symbol level of every dibit in `bits` (one level per two bits), for the trellis decoder.
    levels: Vec<f32>,
    codecs: Codecs,
}

impl Default for P25 {
    fn default() -> Self {
        P25 {
            want: Want::Idle,
            k: 0,
            bits: Vec::with_capacity(2000),
            nac: None,
            idle_syms: 0,
            idens: BTreeMap::new(),
            site: SiteRecord::default(),
            adjacent: BTreeMap::new(),
            call_lc: None,
            enc: None,
            last_end: None,
            pdu_blocks: 0,
            frames: 0,
            header_at: None,
            recent_grants: Vec::new(),
            levels: Vec::with_capacity(1000),
            codecs: Codecs::default(),
        }
    }
}

const KEY: u32 = 1;

impl P25 {
    fn body_len(duid: u8) -> usize {
        match duid {
            duid::HDU => 112 + 648 + 10,
            duid::LDU1 | duid::LDU2 => 1680,
            duid::TDU => 112 + 28,
            duid::TDULC => 112 + 288 + 20,
            duid::TSDU => 112 + 196,
            duid::PDU => 112 + 196,
            _ => 112,
        }
    }

    fn channel_text(ch: u16) -> String {
        format!("{}-{:04X}", ch >> 12, ch & 0xFFF)
    }

    fn channel_freq(&self, ch: u16) -> f64 {
        match self.idens.get(&((ch >> 12) as u8)) {
            Some(i) if i.base_hz > 0.0 => i.base_hz + i.spacing_hz * (ch & 0xFFF) as f64,
            _ => 0.0,
        }
    }

    fn nid(&mut self, ctx: &mut Ctx) {
        let w = to_u64(&self.bits[48..112]);
        let got = decode_nid(&mut self.codecs.nid, w);
        ctx.identity(got.is_some());
        let Some((nac, duid, _)) = got else {
            self.want = Want::Idle;
            return;
        };
        self.frames += 1;
        self.nac = Some(nac);
        self.want = Want::Body(duid, Self::body_len(duid));
        if duid == duid::TSDU || duid == duid::PDU {
            self.pdu_blocks = 0;
        }
    }

    fn access(&self) -> Option<Access> {
        self.nac.map(|n| Access::new(AccessKind::Nac, u32::from(n)))
    }

    fn body(&mut self, duid: u8, ctx: &mut Ctx) {
        let b = std::mem::take(&mut self.bits);
        let lv = std::mem::take(&mut self.levels);
        match duid {
            duid::HDU => self.hdu(&b[112..112 + 648], ctx),
            duid::LDU1 | duid::LDU2 => self.ldu(duid, &b, ctx),
            duid::TDU => self.terminator(None, ctx),
            duid::TDULC => {
                let mut hex = [0u8; 24];
                let mut erasures = [0u16; 24];
                let mut ne = 0;
                let mut fixed = 0;
                for (i, c) in b[112..112 + 288].chunks(24).enumerate() {
                    let d = match TDULC_WORD.decode(to_u64(c) as u128) {
                        Some((cw, e)) => {
                            fixed += e as usize;
                            TDULC_WORD.info(cw) as u16
                        }
                        None => {
                            erasures[ne] = 2 * i as u16;
                            erasures[ne + 1] = 2 * i as u16 + 1;
                            ne += 2;
                            to_u64(&c[..12]) as u16
                        }
                    };
                    hex[2 * i] = (d >> 6) as u8;
                    hex[2 * i + 1] = (d & 0x3F) as u8;
                }
                let ok = self
                    .codecs
                    .rs_24_12
                    .decode(&mut hex, &erasures[..ne])
                    .is_ok();
                ctx.block(ok, 288, fixed);
                let lc = ok.then(|| bits_to_bytes(&hex_to_bits(&hex[..12])));
                self.terminator(lc, ctx);
            }
            duid::TSDU => {
                let mut d = [0u8; 96];
                let r = self.codecs.tcm.decode_levels(&lv[56..154], &mut d);
                let errs = r.map_or(0, |r| r.corrected as usize);
                let bytes = bits_to_bytes(&d);
                let ok = r.is_ok() && CRC16_GSM.verify(&bytes, Trailer::Big);
                ctx.block(ok, 196, errs);
                if ok {
                    self.tsbk(&bytes, ctx);
                    if bytes[0] & 0x80 == 0 && self.pdu_blocks < 2 {
                        // Another TSBK follows.
                        self.pdu_blocks += 1;
                        self.bits.extend_from_slice(&b[..112]);
                        self.levels.extend_from_slice(&lv[..56]);
                        self.want = Want::Body(duid::TSDU, 112 + 196);
                        return;
                    }
                }
            }
            duid::PDU => {
                let mut d = [0u8; 96];
                let r = self.codecs.tcm.decode_levels(&lv[56..154], &mut d);
                let errs = r.map_or(0, |r| r.corrected as usize);
                let bytes = bits_to_bytes(&d);
                let ok = r.is_ok() && CRC16_GSM.verify(&bytes, Trailer::Big);
                ctx.block(ok, 196, errs);
                if ok {
                    self.pdu_header(&bytes, ctx);
                }
            }
            _ => {}
        }
        self.want = Want::Idle;
        self.idle_syms = 0;
    }

    fn hdu(&mut self, b: &[u8], ctx: &mut Ctx) {
        let mut hex = [0u8; 36];
        let mut erasures = [0u16; 36];
        let mut ne = 0;
        let mut fixed = 0;
        for (i, c) in b.chunks(18).take(36).enumerate() {
            hex[i] = match HDU_WORD.decode(to_u64(c) as u128) {
                Some((cw, e)) => {
                    fixed += e as usize;
                    HDU_WORD.info(cw) as u8
                }
                None => {
                    erasures[ne] = i as u16;
                    ne += 1;
                    to_u64(&c[..6]) as u8
                }
            };
        }
        let ok = self
            .codecs
            .rs_36_20
            .decode(&mut hex, &erasures[..ne])
            .is_ok();
        ctx.block(ok, 648, fixed);
        if !ok {
            return;
        }
        let bits = hex_to_bits(&hex[..20]);
        let mfid = to_u64(&bits[72..80]) as u8;
        let alg = to_u64(&bits[80..88]) as u8;
        let kid = to_u64(&bits[88..104]) as u16;
        let tg = to_u64(&bits[104..120]) as u32;
        let _ = mfid;
        self.enc = Some((alg, kid));
        self.header_at = Some(ctx.now);
        if tg != 0 {
            self.start_call(true, 0, tg, false, false, ctx);
        }
    }

    fn start_call(
        &mut self,
        group: bool,
        src: u32,
        dst: u32,
        emergency: bool,
        late: bool,
        ctx: &mut Ctx,
    ) {
        let now = ctx.now;
        if let Some(c) = ctx.call(KEY) {
            // The header carries only the talkgroup: fill the source from the first LC.
            if c.dst == Ident::Number(dst as u64) {
                if src != 0 {
                    ctx.update_call(KEY, |c| {
                        c.src = Ident::Number(src as u64);
                        c.emergency |= emergency;
                        c.group = group;
                    });
                }
                return;
            }
        }
        let enc = self.enc.map(|(alg, kid)| {
            if alg == 0x80 || alg == 0 {
                Encryption {
                    alg_id: Some(alg as u32),
                    alg: alg_name(alg).into(),
                    ..Encryption::clear()
                }
            } else {
                Encryption::encrypted(Some(alg as u32), alg_name(alg), Some(kid as u32))
            }
        });
        let rec = CallRecord {
            kind: CallKind::Voice,
            src: if src == 0 {
                Ident::None
            } else {
                Ident::Number(src as u64)
            },
            dst: Ident::Number(dst as u64),
            group,
            emergency,
            access: self.access(),
            encryption: enc.clone().unwrap_or_default(),
            ..Default::default()
        };
        // A call announced by a header starts at the header.
        let header = self.header_at.filter(|t| now - t < 0.5);
        if let Some(t) = header {
            ctx.now = t;
        }
        ctx.start_call(
            KEY,
            rec,
            late && self.header_at.is_none_or(|t| now - t > 0.5),
        );
        ctx.now = now;
        if let Some(e) = enc {
            ctx.set_encryption(KEY, e);
        }
    }

    fn ldu(&mut self, duid: u8, b: &[u8], ctx: &mut Ctx) {
        // Link control (LDU1) or encryption sync (LDU2).
        let mut hb = Vec::with_capacity(240);
        for &p in &LDU_LC {
            hb.extend_from_slice(&b[p..p + 40]);
        }
        if duid == duid::LDU1 {
            let (hex, ok, fixed) = decode_ldu_hexbits(&hb, &mut self.codecs.rs_24_12);
            ctx.block(ok, 240, fixed);
            if ok {
                let lc = bits_to_bytes(&hex_to_bits(&hex));
                self.link_control(&lc, ctx);
            }
        } else {
            let (hex, ok, fixed) = decode_ldu_hexbits(&hb, &mut self.codecs.rs_24_16);
            ctx.block(ok, 240, fixed);
            if ok {
                let bits = hex_to_bits(&hex);
                let alg = to_u64(&bits[72..80]) as u8;
                let kid = to_u64(&bits[80..96]) as u16;
                let changed = self.enc != Some((alg, kid));
                self.enc = Some((alg, kid));
                if changed && ctx.has_call(KEY) {
                    let e = if alg == 0x80 {
                        Encryption {
                            alg_id: Some(alg as u32),
                            alg: alg_name(alg).into(),
                            ..Encryption::clear()
                        }
                    } else {
                        Encryption::encrypted(Some(alg as u32), alg_name(alg), Some(kid as u32))
                    };
                    ctx.set_encryption(KEY, e);
                }
            }
        }
        // Low speed data: two cyclic (16,8) words.
        for k in 0..2 {
            let got = LSD_WORD.decode(to_u64(&b[LDU_LSD + 16 * k..LDU_LSD + 16 * k + 16]) as u128);
            ctx.block(got.is_some(), 16, got.map_or(0, |(_, e)| e as usize));
        }
        // Voice.
        if ctx.has_call(KEY) {
            ctx.update_call(KEY, |c| c.voice_frames += 9);
            for (i, &p) in LDU_VOICE.iter().enumerate() {
                let cw = crate::voice::channel_bits::<18>(&b[p..p + 144]);
                let t = ctx.now + i as f64 * 0.02;
                let spec = crate::voice::FrameSpec {
                    codec: Codec::Imbe,
                    packaging: crate::voice::IMBE_PACKAGING,
                    codec_mode: None,
                    duration_ms: 20,
                    stack: "p25",
                    carrier_hz: ctx.carrier_hz,
                    slot: 0,
                };
                ctx.speech(KEY, spec, &cw, t, false, None);
            }
        }
    }

    fn link_control(&mut self, lc: &[u8], ctx: &mut Ctx) {
        let lcf = lc[0] & 0x3F;
        let mfid = lc[1];
        if mfid != 0 && mfid != 1 {
            return;
        }
        match lcf {
            0x00 => {
                let opts = lc[2];
                let tg = u16::from_be_bytes([lc[4], lc[5]]) as u32;
                let src = u32::from_be_bytes([0, lc[6], lc[7], lc[8]]);
                self.start_call(true, src, tg, opts & 0x80 != 0, true, ctx);
            }
            0x03 => {
                let opts = lc[2];
                let dst = u32::from_be_bytes([0, lc[3], lc[4], lc[5]]);
                let src = u32::from_be_bytes([0, lc[6], lc[7], lc[8]]);
                self.start_call(false, src, dst, opts & 0x80 != 0, true, ctx);
            }
            _ => {}
        }
    }

    fn terminator(&mut self, lc: Option<Vec<u8>>, ctx: &mut Ctx) {
        let _ = lc;
        if ctx.has_call(KEY) {
            ctx.end_call(KEY, true);
            self.last_end = Some((0, ctx.now));
        } else if let Some((_, t)) = self.last_end {
            // A repeated terminator moves the end of the call it closed.
            let now = ctx.now;
            if now - t < 1.0 {
                let id = ctx
                    .tracker
                    .calls()
                    .newest_first()
                    .find(|c| c.record.kind == CallKind::Voice)
                    .map(|c| c.id);
                if let Some(id) = id {
                    ctx.extend_call(id);
                }
                self.last_end = Some((0, now));
            }
        }
        self.enc = None;
        self.call_lc = None;
        self.header_at = None;
    }

    fn tsbk(&mut self, b: &[u8], ctx: &mut Ctx) {
        let op = b[0] & 0x3F;
        let mfid = b[1];
        if mfid != 0 && mfid != 1 {
            ctx.control(ControlRecord {
                opcode: op as u32,
                mfid: mfid as u32,
                name: "manufacturer".into(),
                text: format!("TSBK {op:02X} manufacturer {mfid:02X}"),
                time: ctx.now,
                ..Default::default()
            });
            return;
        }
        let a = &b[2..10];
        let (name, text): (&str, String) = match op {
            0x00 => {
                let opts = a[0];
                let ch = u16::from_be_bytes([a[1], a[2]]);
                let tg = u16::from_be_bytes([a[3], a[4]]) as u32;
                let src = u32::from_be_bytes([0, a[5], a[6], a[7]]);
                self.grant(true, tg, src, ch, opts, false, ctx);
                (
                    "GRP_V_CH_GRANT",
                    format!(
                        "group voice grant TG {tg} from {src} on {}",
                        Self::channel_text(ch)
                    ),
                )
            }
            0x02 => {
                let ch1 = u16::from_be_bytes([a[0], a[1]]);
                let g1 = u16::from_be_bytes([a[2], a[3]]) as u32;
                let ch2 = u16::from_be_bytes([a[4], a[5]]);
                let g2 = u16::from_be_bytes([a[6], a[7]]) as u32;
                self.grant(true, g1, 0, ch1, 0, true, ctx);
                if g2 != g1 {
                    self.grant(true, g2, 0, ch2, 0, true, ctx);
                }
                (
                    "GRP_V_CH_GRANT_UPDT",
                    format!(
                        "grant update TG {g1} on {}, TG {g2} on {}",
                        Self::channel_text(ch1),
                        Self::channel_text(ch2)
                    ),
                )
            }
            0x04 => {
                let ch = u16::from_be_bytes([a[0], a[1]]);
                let dst = u32::from_be_bytes([0, a[2], a[3], a[4]]);
                let src = u32::from_be_bytes([0, a[5], a[6], a[7]]);
                self.grant(false, dst, src, ch, 0, false, ctx);
                (
                    "UU_V_CH_GRANT",
                    format!(
                        "unit to unit grant {src} -> {dst} on {}",
                        Self::channel_text(ch)
                    ),
                )
            }
            0x34 => {
                let id = a[0] >> 4;
                let bw = a[0] & 0x0F;
                let sign = a[1] & 0x80 != 0;
                let off = (u16::from_be_bytes([a[1], a[2]]) >> 2) & 0x1FFF;
                let spacing = (u16::from_be_bytes([a[2], a[3]]) & 0x3FF) as f64 * 125.0;
                let base = u32::from_be_bytes([a[4], a[5], a[6], a[7]]) as f64 * 5.0;
                let tx = off as f64 * spacing * if sign { 1.0 } else { -1.0 };
                let _ = bw;
                self.idens.insert(
                    id,
                    Iden {
                        id,
                        vhf_uhf: true,
                        base_hz: base,
                        spacing_hz: spacing,
                        tx_offset_hz: tx,
                        bandwidth_hz: 0.0,
                    },
                );
                (
                    "IDEN_UP_VU",
                    format!(
                        "identifier {id}: base {:.5} MHz, spacing {:.2} kHz, offset {:+.3} MHz",
                        base / 1e6,
                        spacing / 1e3,
                        tx / 1e6
                    ),
                )
            }
            0x3D => {
                let id = a[0] >> 4;
                let bw = (u16::from_be_bytes([a[0], a[1]]) >> 3) & 0x1FF;
                let sign = a[1] & 0x04 != 0;
                let off = ((u16::from_be_bytes([a[1], a[2]]) >> 2) & 0xFF) as f64;
                let spacing = (u16::from_be_bytes([a[2], a[3]]) & 0x3FF) as f64 * 125.0;
                let base = u32::from_be_bytes([a[4], a[5], a[6], a[7]]) as f64 * 5.0;
                let tx = off * 250_000.0 * if sign { 1.0 } else { -1.0 };
                self.idens.insert(
                    id,
                    Iden {
                        id,
                        vhf_uhf: false,
                        base_hz: base,
                        spacing_hz: spacing,
                        tx_offset_hz: tx,
                        bandwidth_hz: bw as f64 * 125.0,
                    },
                );
                (
                    "IDEN_UP",
                    format!(
                        "identifier {id}: base {:.5} MHz, spacing {:.2} kHz, offset {:+.3} MHz",
                        base / 1e6,
                        spacing / 1e3,
                        tx / 1e6
                    ),
                )
            }
            0x3A => {
                let sys = u16::from_be_bytes([a[1], a[2]]) & 0xFFF;
                let rfss = a[3];
                let site = a[4];
                let ch = u16::from_be_bytes([a[5], a[6]]);
                self.site.system = Some(u32::from(sys));
                self.site.rfss = Some(u32::from(rfss));
                self.site.site = Some(u32::from(site));
                self.site.control_channel = Some(Self::channel_text(ch));
                self.publish_site(ctx);
                (
                    "RFSS_STS_BCST",
                    format!(
                        "RFSS status: system {sys:03X} RFSS {rfss} site {site} on {}",
                        Self::channel_text(ch)
                    ),
                )
            }
            0x3B => {
                let wacn = (u32::from_be_bytes([0, a[1], a[2], a[3]]) >> 4) & 0xFFFFF;
                let sys = u16::from_be_bytes([a[3], a[4]]) & 0xFFF;
                let ch = u16::from_be_bytes([a[5], a[6]]);
                self.site.wacn = Some(wacn);
                self.site.system = Some(u32::from(sys));
                self.publish_site(ctx);
                (
                    "NET_STS_BCST",
                    format!(
                        "network status: WACN {wacn:05X} system {sys:03X} on {}",
                        Self::channel_text(ch)
                    ),
                )
            }
            0x3C => {
                let sys = u16::from_be_bytes([a[1], a[2]]) & 0xFFF;
                let rfss = a[3];
                let site = a[4];
                let ch = u16::from_be_bytes([a[5], a[6]]);
                let m = Adjacent {
                    system: sys,
                    rfss,
                    site,
                    channel: Self::channel_text(ch),
                    freq_hz: self.channel_freq(ch),
                };
                self.adjacent.insert(((rfss as u32) << 8) | site as u32, m);
                (
                    "ADJ_STS_BCST",
                    format!(
                        "adjacent site: RFSS {rfss} site {site} on {}",
                        Self::channel_text(ch)
                    ),
                )
            }
            _ => ("TSBK", format!("TSBK {op:02X}")),
        };
        ctx.control(ControlRecord {
            opcode: op as u32,
            mfid: mfid as u32,
            name: name.into(),
            text,
            time: ctx.now,
            slot: 0,
        });
    }

    fn publish_site(&mut self, ctx: &mut Ctx) {
        if let Some(n) = self.nac {
            self.site.nac = Some(u32::from(n));
        }
        self.site.valid = self.site.wacn.is_some() && self.site.site.is_some();
        self.site.time = ctx.now;
        ctx.site(self.site.clone());
    }

    #[allow(clippy::too_many_arguments)]
    fn grant(
        &mut self,
        group: bool,
        target: u32,
        src: u32,
        ch: u16,
        opts: u8,
        update: bool,
        ctx: &mut Ctx,
    ) {
        let freq = self.channel_freq(ch);
        let g = GrantRecord {
            target: target as u64,
            group,
            src: src as u64,
            channel: ch as u32,
            channel_text: Self::channel_text(ch),
            freq_hz: freq,
            slot: None,
            emergency: opts & 0x80 != 0,
            encrypted: opts & 0x40 != 0,
            priority: if opts & 7 == 0 {
                4
            } else {
                (8 - (opts & 7)).clamp(1, 7)
            },
            update,
            time: ctx.now,
        };
        ctx.grant(g);
        if update {
            return;
        }
        // A grant repeated on the control channel is one entry.
        let now = ctx.now;
        self.recent_grants.retain(|r| now - r.3 < 5.0);
        if let Some(r) = self
            .recent_grants
            .iter_mut()
            .find(|r| r.0 == target && r.1 == ch && r.2 == src)
        {
            r.3 = now;
            return;
        }
        self.recent_grants.push((target, ch, src, now));
        let mut rec = CallRecord {
            kind: CallKind::Grant,
            src: if src == 0 {
                Ident::None
            } else {
                Ident::Number(src as u64)
            },
            dst: Ident::Number(target as u64),
            group,
            emergency: opts & 0x80 != 0,
            access: self.access(),
            ..Default::default()
        };
        rec.set(CallDetail::Channel, Value::Text(Self::channel_text(ch)));
        rec.set(CallDetail::FreqHz, Value::Hz(freq));
        ctx.log_single(rec);
    }

    fn pdu_header(&mut self, b: &[u8], ctx: &mut Ctx) {
        let llid = u32::from_be_bytes([0, b[3], b[4], b[5]]);
        let sap = b[1] & 0x3F;
        let mut rec = CallRecord {
            kind: CallKind::Data,
            dst: Ident::Number(llid as u64),
            access: self.access(),
            ..Default::default()
        };
        rec.set(CallDetail::Sap, Value::I64(i64::from(sap)));
        rec.set(CallDetail::Blocks, Value::I64(i64::from(b[6] & 0x7F)));
        ctx.log_single(rec);
    }
}

/// An adjacent site.
#[derive(Clone, Debug, PartialEq)]
struct Adjacent {
    system: u16,
    rfss: u8,
    site: u8,
    channel: String,
    freq_hz: f64,
}

impl Protocol for P25 {
    fn params(&self) -> ModeParams {
        ModeParams {
            mode: "p25",
            name: "P25",
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
        vec![SyncPattern::from_hex_dibits(0, FS, 0).with_max_err(3)]
    }

    fn on_sync(
        &mut self,
        _id: u8,
        _inv: bool,
        pre: &[Sym],
        _at: u64,
        _tracking: bool,
        _ctx: &mut Ctx,
    ) -> SyncAnswer {
        self.bits.clear();
        self.levels.clear();
        for s in pre {
            let d = s.dibit();
            self.bits.push(d >> 1);
            self.bits.push(d & 1);
            self.levels.push(s.v);
        }
        self.k = pre.len();
        self.want = Want::Nid;
        SyncAnswer::Take
    }

    fn on_symbol(&mut self, s: Sym, _at: u64, ctx: &mut Ctx) -> SymAnswer {
        let k = self.k;
        self.k += 1;
        if self.want == Want::Idle {
            self.idle_syms += 1;
            return if self.idle_syms > 900 {
                SymAnswer::Stop
            } else {
                SymAnswer::Continue
            };
        }
        // Status symbol: every 36th symbol of the frame.
        if k % 36 == 35 {
            return SymAnswer::Continue;
        }
        let d = s.dibit();
        self.bits.push(d >> 1);
        self.bits.push(d & 1);
        self.levels.push(s.v);
        match self.want {
            Want::Nid if self.bits.len() >= 112 => self.nid(ctx),
            Want::Body(duid, n) if self.bits.len() >= n => {
                self.body(duid, ctx);
            }
            _ => {}
        }
        SymAnswer::Continue
    }

    fn fields(&self) -> Vec<&'static dyn Declare> {
        <P25Fields as Fields>::SHAPE.fields.to_vec()
    }

    fn tables(&self) -> Vec<&'static TableDef> {
        vec![IDENTIFIERS.def(), ADJACENT.def()]
    }

    fn publish(&self, r: &mut Published) {
        r.put(&P25Fields {
            nac: self.nac.map(i64::from),
            p25_frames: self.frames as i64,
        });
        let mut key = String::new();
        for i in self.idens.values() {
            key.clear();
            // Writing to a `String` cannot fail.
            let _ = write!(key, "{}", i.id);
            let row = IdentifierRow {
                id: i64::from(i.id),
                vhf_uhf: i.vhf_uhf,
                base_hz: Hertz(i.base_hz),
                spacing_hz: Hertz(i.spacing_hz),
                tx_offset_hz: Hertz(i.tx_offset_hz),
                bandwidth_hz: Hertz(i.bandwidth_hz),
            };
            IDENTIFIERS.put(r, &key, &row);
        }
        for (k, m) in &self.adjacent {
            key.clear();
            let _ = write!(key, "{k}");
            let row = AdjacentRow {
                system: i64::from(m.system),
                rfss: i64::from(m.rfss),
                site: i64::from(m.site),
                channel: &m.channel,
                freq_hz: (m.freq_hz > 0.0).then_some(Hertz(m.freq_hz)),
            };
            ADJACENT.put(r, &key, &row);
        }
    }

    fn codec(&self) -> Codec {
        Codec::Imbe
    }
}

// ---------------------------------------------------------------------------------------------------------------------------
// Generator.

struct Frames<'a> {
    nac: u16,
    bits: Vec<u8>,
    codecs: Codecs,
    /// The voice frames to send in turn (144 channel bits each, as sent); pseudo-random bits when empty.
    speech: &'a [[u8; 144]],
    next_speech: usize,
}

type Rng = sdrkit_algo_sim::rng::Rng;

/// The 196 bits of a rate 1/2 trellis block of 12 bytes.
fn trellis_half(codecs: &mut Codecs, t: &[u8; 12]) -> [u8; 196] {
    let mut out = [0u8; 196];
    codecs
        .tcm
        .encode(&bytes_to_bits(t), &mut out)
        .expect("96 bits in");
    out
}

impl Frames<'_> {
    fn header(&mut self, duid: u8) -> Vec<u8> {
        let mut d = Vec::with_capacity(1700);
        push_u64(&mut d, u64::from_str_radix(FS, 16).unwrap_or(0), 48);
        push_u64(&mut d, encode_nid(self.nac, duid), 64);
        d
    }
    fn push(&mut self, data: Vec<u8>) {
        let mut f = insert_status(&data, 2);
        // Pad to a whole number of symbols.
        if f.len() % 2 == 1 {
            f.push(0);
        }
        self.bits.extend(f);
    }
    fn tsdu(&mut self, blocks: &[[u8; 10]]) {
        let mut d = self.header(duid::TSDU);
        for (i, b) in blocks.iter().enumerate() {
            let mut t = [0u8; 12];
            t[..10].copy_from_slice(b);
            if i == blocks.len() - 1 {
                t[0] |= 0x80;
            }
            let crc = CRC16_GSM.checksum(&t[..10]);
            t[10..].copy_from_slice(&crc.to_be_bytes());
            d.extend(trellis_half(&mut self.codecs, &t));
        }
        self.push(d);
    }
    fn hdu(&mut self, alg: u8, kid: u16, tg: u16) {
        let mut d = self.header(duid::HDU);
        let mut bits = vec![0u8; 72];
        push_u64(&mut bits, 0, 8);
        push_u64(&mut bits, alg as u64, 8);
        push_u64(&mut bits, kid as u64, 16);
        push_u64(&mut bits, tg as u64, 16);
        let mut w = bits_to_hex(&bits);
        w.resize(36, 0);
        self.codecs.rs_36_20.encode_word(&mut w).expect("RS sizes");
        for h in w {
            push_u64(&mut d, HDU_WORD.encode(h as u128) as u64, 18);
        }
        d.extend([0u8; 10]);
        self.push(d);
    }
    fn ldu(&mut self, duid: u8, hexbits: Vec<u8>, rng: &mut Rng) {
        let mut d = self.header(duid);
        d.resize(1680, 0);
        for &p in &LDU_VOICE {
            if self.speech.is_empty() {
                for i in 0..144 {
                    d[p + i] = rng.bit();
                }
            } else {
                d[p..p + 144].copy_from_slice(&self.speech[self.next_speech % self.speech.len()]);
                self.next_speech += 1;
            }
        }
        for (k, &p) in LDU_LC.iter().enumerate() {
            d[p..p + 40].copy_from_slice(&hexbits[k * 40..k * 40 + 40]);
        }
        let mut lsd = Vec::new();
        push_u64(&mut lsd, LSD_WORD.encode(0) as u64, 16);
        push_u64(&mut lsd, LSD_WORD.encode(0) as u64, 16);
        d[LDU_LSD..LDU_LSD + 32].copy_from_slice(&lsd);
        self.push(d);
    }
    #[allow(clippy::too_many_arguments)]
    fn voice(
        &mut self,
        group: bool,
        src: u32,
        dst: u32,
        emergency: bool,
        alg: u8,
        kid: u16,
        ldus: usize,
        rng: &mut Rng,
    ) {
        self.hdu(alg, kid, if group { dst as u16 } else { 0 });
        let opts = ((emergency as u8) << 7) | (((alg != 0x80) as u8) << 6);
        let lc: [u8; 9] = if group {
            let t = (dst as u16).to_be_bytes();
            let s = src.to_be_bytes();
            [0x00, 0x00, opts, 0, t[0], t[1], s[1], s[2], s[3]]
        } else {
            let t = dst.to_be_bytes();
            let s = src.to_be_bytes();
            [0x03, 0x00, opts, t[1], t[2], t[3], s[1], s[2], s[3]]
        };
        let lc_hex = bits_to_hex(&bytes_to_bits(&lc));
        let lc_bits = encode_ldu_hexbits(&lc_hex, &self.codecs.rs_24_12);
        let mut es = vec![0u8; 72];
        push_u64(&mut es, alg as u64, 8);
        push_u64(&mut es, kid as u64, 16);
        let es_bits = encode_ldu_hexbits(&bits_to_hex(&es), &self.codecs.rs_24_16);
        for i in 0..ldus {
            if i % 2 == 0 {
                self.ldu(duid::LDU1, lc_bits.clone(), rng);
            } else {
                self.ldu(duid::LDU2, es_bits.clone(), rng);
            }
        }
        // Terminator with LC.
        let mut d = self.header(duid::TDULC);
        let mut w = lc_hex.clone();
        w.resize(24, 0);
        self.codecs.rs_24_12.encode_word(&mut w).expect("RS sizes");
        for pair in w.chunks(2) {
            let v = ((pair[0] as u128) << 6) | pair[1] as u128;
            push_u64(&mut d, TDULC_WORD.encode(v) as u64, 24);
        }
        d.extend([0u8; 20]);
        self.push(d);
        let mut d = self.header(duid::TDU);
        d.extend([0u8; 28]);
        self.push(d);
    }
}

fn tsbk_args(op: u8, args: [u8; 8]) -> [u8; 10] {
    let mut b = [0u8; 10];
    b[0] = op;
    b[2..].copy_from_slice(&args);
    b
}

/// The P25 test signal: a control channel with site, identifiers and a grant, then voice calls (one encrypted), a data header
/// and an emergency unit-to-unit call. The voice frames are pseudo-random bits.
pub fn render(nac: u16, seed: u64) -> Vec<Complex32> {
    render_with_speech(nac, seed, &[])
}

/// The P25 test signal of [`render`] whose voice frames are those of `speech` in turn (144 channel bits each, as sent), or
/// pseudo-random bits when it is empty.
pub fn render_with_speech(nac: u16, seed: u64, speech: &[[u8; 144]]) -> Vec<Complex32> {
    let mut f = Frames {
        nac,
        bits: Vec::new(),
        codecs: Codecs::default(),
        speech,
        next_speech: 0,
    };
    let mut rng = Rng::new(seed);
    // Control channel: 3 x (network, RFSS, adjacent. Identifiers. Grant).
    let wacn = 0xBEE00u32;
    let sys = 0x3A1u16;
    let net = {
        let w = (wacn << 4) | (sys as u32 >> 8);
        tsbk_args(
            0x3B,
            [
                0,
                (w >> 16) as u8,
                (w >> 8) as u8,
                w as u8,
                sys as u8,
                0x10,
                0x01,
                0x70,
            ],
        )
    };
    let rfss = tsbk_args(
        0x3A,
        [0, (sys >> 8) as u8, sys as u8, 1, 5, 0x10, 0x01, 0x70],
    );
    let adj = tsbk_args(
        0x3C,
        [0, (sys >> 8) as u8, sys as u8, 1, 6, 0x10, 0x09, 0x70],
    );
    // Identifier 1: base 851.00625 MHz, 12.5 kHz, -45 MHz. Identifier 2 (VHF/UHF): base 460 MHz, 12.5 kHz, +5 MHz (400 steps).
    let iden1 = {
        let base = (851_006_250u64 / 5) as u32;
        let bw: u16 = 100; // 12.5 kHz in 125 Hz
        let off: u16 = 180; // 45 MHz / 250 kHz, negative
        let sp: u16 = 100;
        let w: u64 = (1u64 << 60)
            | ((bw as u64) << 51)
            | ((off as u64) << 42)
            | ((sp as u64) << 32)
            | base as u64;
        let mut a = [0u8; 8];
        a.copy_from_slice(&w.to_be_bytes());
        tsbk_args(0x3D, a)
    };
    let iden2 = {
        let base = (460_000_000u64 / 5) as u32;
        let sp: u16 = 100;
        let off: u16 = 400; // 400 x 12.5 kHz = 5 MHz, positive
        let w: u64 = (2u64 << 60)
            | (5u64 << 56)
            | (1u64 << 55)
            | ((off as u64) << 42)
            | ((sp as u64) << 32)
            | base as u64;
        let mut a = [0u8; 8];
        a.copy_from_slice(&w.to_be_bytes());
        tsbk_args(0x34, a)
    };
    let grant = {
        let ch: u16 = (1 << 12) | 0x010;
        let tg: u16 = 1201;
        let s = 1234567u32.to_be_bytes();
        tsbk_args(
            0x00,
            [
                0,
                (ch >> 8) as u8,
                ch as u8,
                (tg >> 8) as u8,
                tg as u8,
                s[1],
                s[2],
                s[3],
            ],
        )
    };
    for _ in 0..3 {
        f.tsdu(&[net, rfss, adj]);
        f.tsdu(&[iden1, iden2, grant]);
    }
    let mut out = Vec::new();
    let mut m = FskModulator::new(4800.0, TxPulse::RaisedCosine(0.2), 600.0);
    silence(0.3, &mut out);
    let emit = |f: &mut Frames, out: &mut Vec<Complex32>, m: &mut FskModulator, gap: f64| {
        m.symbols(&bits_to_levels(&f.bits), out);
        m.flush(8, out);
        m.reset();
        f.bits.clear();
        silence(gap, out);
    };
    emit(&mut f, &mut out, &mut m, 0.5);
    f.voice(true, 1234567, 1201, false, 0x80, 0, 12, &mut rng);
    emit(&mut f, &mut out, &mut m, 0.5);
    f.voice(true, 7654321, 1300, false, 0x84, 0x1234, 8, &mut rng);
    emit(&mut f, &mut out, &mut m, 0.5);
    // A packet data header for LLID 2002.
    {
        let mut d = f.header(duid::PDU);
        let mut h = [0u8; 12];
        h[0] = 0x15;
        h[1] = 0xC0 | 0x04;
        h[2] = 0;
        h[3..6].copy_from_slice(&2002u32.to_be_bytes()[1..]);
        h[6] = 0x80;
        let crc = CRC16_GSM.checksum(&h[..10]);
        h[10..].copy_from_slice(&crc.to_be_bytes());
        d.extend(trellis_half(&mut f.codecs, &h));
        f.push(d);
    }
    emit(&mut f, &mut out, &mut m, 0.5);
    f.voice(false, 2002, 2003, true, 0x80, 0, 6, &mut rng);
    emit(&mut f, &mut out, &mut m, 0.5);
    out
}

/// The NAC of the test signal.
pub const GEN_NAC: sdrkit_core::Setting<field::I64> =
    sdrkit_core::Setting::new("gen_nac", "NAC", "", 659)
        .range(0.0, 4095.0, 1.0)
        .section("Test signal")
        .advanced();

/// The P25 test signal of the settings in `cfg` ([`GEN_NAC`]).
pub fn generator(cfg: &GenConfig) -> Prepared {
    let nac = cfg.settings.get(&GEN_NAC) as u16;
    Prepared::new(render(nac, cfg.seed), cfg.rate_hz, true).with_info(12_500.0, 4800.0, 9600.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nid_and_hexbit_codes_round_trip() {
        let mut c = Codecs::default();
        let w = encode_nid(0x293, duid::LDU1);
        assert_eq!(
            decode_nid(&mut c.nid, w ^ 0x8000_0000_0010_0201),
            Some((0x293, 5, 4))
        );
        let hex: Vec<u8> = (0..12).map(|i| (i * 5 + 3) as u8 & 0x3F).collect();
        let mut bits = encode_ldu_hexbits(&hex, &c.rs_24_12);
        bits[3] ^= 1;
        bits[77] ^= 1;
        let (got, ok, _) = decode_ldu_hexbits(&bits, &mut c.rs_24_12);
        assert!(ok);
        assert_eq!(got, hex);
    }

    #[test]
    fn status_symbols_sit_every_70_bits() {
        let d = vec![1u8; 140];
        let f = insert_status(&d, 2);
        assert_eq!(f.len(), 144);
        assert_eq!((f[70], f[71], f[142], f[143]), (1, 0, 1, 0));
    }
}
