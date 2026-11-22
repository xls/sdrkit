//! P25 Phase 2: the outbound H-DQPSK channel, two TDMA channels in a 360 ms superframe, the slot information, the burst types
//! and the MAC messages of a call, with the AMBE+2 voice of each channel.
//! Standard: TIA-102.BBAB, TIA-102.BBAC.

use crate::bits::{bits_to_bytes, bytes_to_bits};
use crate::decoder::{Ctx, ModeParams, Protocol};
use crate::gen::Prepared;
use crate::items::*;
use crate::symbols::{Sym, SymAnswer, SyncAnswer, SyncPattern};
use crate::typed::P25p2Fields;
use sdrkit_algo_fec::rs::{codes as rs_codes, Rs};
use sdrkit_algo_fec::small::SmallCode;
use sdrkit_core::interface::Fields;
use sdrkit_core::media::Codec;
use sdrkit_core::publish::Published;
use sdrkit_core::publish::Declare;
use sdrkit_core::{Complex32, GenConfig};
use sdrkit_util_bits::pack::{push_u64, to_u64};
use std::sync::LazyLock;

/// The sync of the S-ISCH.
pub const SYNC: &str = "575D57F7FF";
/// Bits per timeslot.
pub const SLOT_BITS: usize = 360;

/// Burst types.
pub mod duid {
    pub const V4: u8 = 0;
    pub const V2: u8 = 3;
    pub const FACCH: u8 = 6;
    pub const SACCH: u8 = 9;
    pub const IDLE: u8 = 12;
}

/// The (8,4) code of the DUID: the generator rows of the 4 burst type bits, the first check bit most significant.
const DUID_ROWS: [u32; 4] = [0b1110, 0b1101, 0b1011, 0b0111];

/// The (8,4) code of the DUID field.
static DUID: LazyLock<SmallCode> =
    LazyLock::new(|| SmallCode::from_parity_rows(8, 4, &DUID_ROWS).expect("a valid (8,4) code"));
/// Extended Golay (24,12) of the I-ISCH.
static ISCH: LazyLock<SmallCode> = LazyLock::new(SmallCode::golay_24_12);

/// The RS decoders a Phase 2 receiver keeps.
#[derive(Clone, Debug)]
pub struct Codecs {
    /// RS(45,26) of the FACCH.
    pub facch: Rs,
    /// RS(52,30) of the SACCH.
    pub sacch: Rs,
}

impl Default for Codecs {
    fn default() -> Self {
        Codecs {
            facch: Rs::new(rs_codes::P25_45_26).expect("a valid RS code"),
            sacch: Rs::new(rs_codes::P25_52_30).expect("a valid RS code"),
        }
    }
}

fn hex_of(bits: &[u8]) -> Vec<u8> {
    bits.chunks(6).map(|c| to_u64(c) as u8).collect()
}
fn bits_of_hex(hex: &[u8]) -> Vec<u8> {
    let mut b = Vec::with_capacity(hex.len() * 6);
    for &h in hex {
        push_u64(&mut b, h as u64, 6);
    }
    b
}

/// The 8-bit DUID field: the 4-bit burst type with its 4 parity bits (a (8,4) extended Hamming code).
pub fn encode_duid(d: u8) -> u8 {
    DUID.encode(d as u128 & 0xF) as u8
}

/// The burst type of a received DUID field, with one bit corrected. `None` beyond.
pub fn decode_duid(w: u8) -> Option<u8> {
    let (cw, _) = DUID.decode(w as u128)?;
    Some(DUID.info(cw) as u8)
}

/// The slot information of the I-ISCH: location in the superframe (0..11) and channel, a Golay (24,12) word repeated with a
/// 16-bit complement tail.
pub fn encode_isch(location: u8, channel: u8) -> Vec<u8> {
    let info = ((location as u128 & 0xF) << 4) | ((channel as u128 & 1) << 3);
    let mut b = Vec::with_capacity(40);
    push_u64(&mut b, ISCH.encode(info) as u64, 24);
    let tail: Vec<u8> = b[..16].iter().map(|x| 1 - x).collect();
    b.extend(tail);
    b
}

pub fn decode_isch(b: &[u8]) -> Option<(u8, u8)> {
    let (cw, _) = ISCH.decode(to_u64(b.get(..24)?) as u128)?;
    let d = ISCH.info(cw);
    Some((((d >> 4) & 0xF) as u8, ((d >> 3) & 1) as u8))
}

/// MAC opcodes.
pub mod mac {
    pub const PTT: u8 = 1;
    pub const END_PTT: u8 = 2;
    pub const IDLE: u8 = 3;
    pub const ACTIVE: u8 = 4;
    pub const HANGTIME: u8 = 6;
}

#[derive(Clone, Copy, Debug, Default)]
struct Chan {
    in_call: bool,
}

/// The Phase 2 protocol.
pub struct P25p2 {
    buf: Vec<Sym>,
    in_slot: bool,
    location: u8,
    chans: [Chan; 2],
    site: SiteRecord,
    idle: usize,
    synced: bool,
    codecs: Codecs,
}

impl Default for P25p2 {
    fn default() -> Self {
        P25p2 {
            buf: Vec::with_capacity(200),
            in_slot: false,
            location: 0,
            chans: [Chan::default(); 2],
            site: SiteRecord::default(),
            idle: 0,
            synced: false,
            codecs: Codecs::default(),
        }
    }
}

fn key(ch: u8) -> u32 {
    ch as u32 + 1
}

impl P25p2 {
    fn slot(&mut self, ctx: &mut Ctx) {
        let mut bits = Vec::with_capacity(SLOT_BITS);
        for s in &self.buf {
            let d = s.dibit();
            bits.push(d >> 1);
            bits.push(d & 1);
        }
        let sync = u64::from_str_radix(SYNC, 16).expect("a constant hex word");
        let isch = to_u64(&bits[..40]);
        // The slot is in its place when its S-ISCH or I-ISCH reads.
        let placed = if (isch ^ sync).count_ones() <= 4 {
            // S-ISCH: the slot after it is location 0 of a half superframe (0 or 6).
            self.location = if self.location < 6 { 5 } else { 11 };
            true
        } else if let Some((loc, _)) = decode_isch(&bits[..40]) {
            self.location = loc;
            true
        } else {
            self.location = (self.location + 1) % 12;
            false
        };
        let ch = self.location & 1;
        let d = decode_duid(to_u64(&bits[40..48]) as u8);
        ctx.identity(d.is_some());
        let Some(d) = d else {
            return;
        };
        let payload = &bits[48..SLOT_BITS];
        match d {
            duid::V4 | duid::V2 => {
                // The voice codewords carry no error check: a voice slot in its place counts as decoded.
                if placed {
                    ctx.confirmed();
                }
                let n = if d == duid::V4 { 4 } else { 2 };
                let k = key(ch);
                if ctx.has_call(k) {
                    ctx.update_call(k, |c| c.voice_frames += n as u32);
                    for i in 0..n {
                        let cw = sdrkit_codec_mbe::codewords(&payload[i * 72..i * 72 + 72]);
                        let spec = crate::voice::FrameSpec {
                            codec: Codec::Ambe2,
                            packaging: sdrkit_graph::SpeechPackaging::Codewords,
                            codec_mode: None,
                            duration_ms: 20,
                            stack: "p25p2",
                            carrier_hz: ctx.carrier_hz,
                            slot: ch,
                        };
                        ctx.speech(k, spec, &cw, ctx.now + i as f64 * 0.02, false, None);
                    }
                }
            }
            duid::FACCH => {
                let mut w = hex_of(&payload[..270]);
                let ok = self.codecs.facch.decode(&mut w, &[]).ok();
                ctx.block(
                    ok.is_some(),
                    270,
                    ok.map_or(0, |d| d.corrected as usize * 6),
                );
                if ok.is_some() {
                    let m = bits_to_bytes(&bits_of_hex(&w[..26]));
                    self.mac(&m, ch, ctx);
                }
            }
            duid::SACCH => {
                let mut w = hex_of(&payload[..312]);
                let ok = self.codecs.sacch.decode(&mut w, &[]).ok();
                ctx.block(
                    ok.is_some(),
                    312,
                    ok.map_or(0, |d| d.corrected as usize * 6),
                );
                if ok.is_some() {
                    let m = bits_to_bytes(&bits_of_hex(&w[..30]));
                    self.mac(&m, 1 - ch, ctx);
                }
            }
            _ => {}
        }
    }

    /// A MAC PDU: opcode (3 bits) and offset in the first octet, then the message.
    fn mac(&mut self, m: &[u8], ch: u8, ctx: &mut Ctx) {
        let op = m[0] >> 5;
        let k = key(ch);
        match op {
            mac::PTT => {
                let alg = m[10];
                let kid = u16::from_be_bytes([m[11], m[12]]);
                let src = u32::from_be_bytes([0, m[13], m[14], m[15]]);
                let tg = u16::from_be_bytes([m[16], m[17]]) as u32;
                if let Some(c) = ctx.call(k) {
                    if c.src == Ident::Number(src as u64) && c.dst == Ident::Number(tg as u64) {
                        return;
                    }
                }
                let enc = if alg == 0x80 || alg == 0 {
                    Encryption {
                        alg_id: Some(alg as u32),
                        alg: crate::p25::alg_name(alg).into(),
                        ..Encryption::clear()
                    }
                } else {
                    Encryption::encrypted(
                        Some(alg as u32),
                        crate::p25::alg_name(alg),
                        Some(kid as u32),
                    )
                };
                let rec = CallRecord {
                    kind: CallKind::Voice,
                    slot: ch + 1,
                    src: Ident::Number(src as u64),
                    dst: Ident::Number(tg as u64),
                    group: true,
                    access: self.site.nac.map(|n| Access::new(AccessKind::Nac, n)),
                    encryption: enc.clone(),
                    ..Default::default()
                };
                ctx.start_call(k, rec, false);
                ctx.set_encryption(k, enc);
                self.chans[ch as usize].in_call = true;
            }
            mac::END_PTT => {
                if ctx.has_call(k) {
                    ctx.end_call(k, true);
                }
                self.chans[ch as usize].in_call = false;
            }
            mac::IDLE | mac::HANGTIME => {}
            mac::ACTIVE => {
                // Network status broadcast in the message: WACN (20), system (12), RFSS, site, NAC (12).
                let w = to_u64(&bytes_to_bits(&m[1..8])[..56]);
                let wacn = (w >> 36) & 0xFFFFF;
                let sys = (w >> 24) & 0xFFF;
                let rfss = (w >> 16) & 0xFF;
                let site = (w >> 8) & 0xFF;
                let nac = u16::from_be_bytes([m[8], m[9]]) & 0xFFF;
                self.site.wacn = Some(wacn as u32);
                self.site.system = Some(sys as u32);
                self.site.rfss = Some(rfss as u32);
                self.site.site = Some(site as u32);
                self.site.nac = Some(u32::from(nac));
                self.site.valid = true;
                self.site.time = ctx.now;
                ctx.site(self.site.clone());
                ctx.control(ControlRecord {
                    opcode: op as u32,
                    name: "NET_STS".into(),
                    text: format!("network status: WACN {wacn:05X} system {sys:03X} site {site}"),
                    time: ctx.now,
                    ..Default::default()
                });
            }
            _ => {}
        }
    }
}

impl Protocol for P25p2 {
    fn params(&self) -> ModeParams {
        ModeParams {
            mode: "p25p2",
            name: "P25 Phase 2",
            baud: 6000.0,
            two_level: false,
            channel_hz: 12_500.0,
            calls: &crate::results::CALLS_300,
            message_cap: 100,
            polarity_rule: false,
            clock_loop: false,
            extra_baud: None,
            stream_rate: crate::front::RATE,
        }
    }

    fn patterns(&self) -> Vec<SyncPattern> {
        vec![SyncPattern::from_hex_dibits(0, SYNC, 0).with_max_err(2)]
    }

    fn on_sync(
        &mut self,
        _id: u8,
        _inv: bool,
        _pre: &[Sym],
        _at: u64,
        tracking: bool,
        _ctx: &mut Ctx,
    ) -> SyncAnswer {
        // The sync is the first 20 symbols of a slot.
        if tracking && self.synced && self.buf.len() > 22 && self.buf.len() < 178 {
            return SyncAnswer::Ignore;
        }
        self.buf.clear();
        self.buf.extend_from_slice(_pre);
        self.in_slot = true;
        self.synced = true;
        self.idle = 0;
        SyncAnswer::Take
    }

    fn on_symbol(&mut self, s: Sym, _at: u64, ctx: &mut Ctx) -> SymAnswer {
        if !self.in_slot {
            self.idle += 1;
            return if self.idle > 1000 {
                SymAnswer::Stop
            } else {
                SymAnswer::Continue
            };
        }
        self.buf.push(s);
        if self.buf.len() == SLOT_BITS / 2 {
            if crate::symbols::carrier_gone(&self.buf) {
                self.in_slot = false;
                self.synced = false;
                for ch in 0..2 {
                    if ctx.has_call(key(ch)) {
                        ctx.end_call(key(ch), false);
                    }
                }
                return SymAnswer::Stop;
            }
            self.slot(ctx);
            self.buf.clear();
        }
        SymAnswer::Continue
    }

    fn on_tick(&mut self, ctx: &mut Ctx) {
        for ch in 0..2 {
            if ctx.has_call(key(ch)) && ctx.now - ctx.last_frame_s > 1.0 {
                ctx.end_call(key(ch), false);
            }
        }
    }

    fn fields(&self) -> Vec<&'static dyn Declare> {
        <P25p2Fields as Fields>::SHAPE.fields.to_vec()
    }

    fn publish(&self, r: &mut Published) {
        r.put(&P25p2Fields {
            location: self.location as i64,
        });
    }

    fn codec(&self) -> Codec {
        Codec::Ambe2
    }
}

// ---------------------------------------------------------------------------------------------------------------------------
// Generator.

fn facch(m: &[u8]) -> Vec<u8> {
    let mut b = bytes_to_bits(m);
    b.resize(156, 0);
    let mut w = hex_of(&b);
    w.resize(45, 0);
    if Codecs::default().facch.encode_word(&mut w).is_err() {
        w.fill(0);
    }
    let mut out = bits_of_hex(&w);
    out.resize(312, 0);
    out
}

fn slot_bits(location: u8, sync: bool, d: u8, payload: &[u8]) -> Vec<u8> {
    let mut b = Vec::with_capacity(SLOT_BITS);
    if sync {
        push_u64(&mut b, u64::from_str_radix(SYNC, 16).unwrap_or(0), 40);
    } else {
        b.extend(encode_isch(location, location & 1));
    }
    push_u64(&mut b, encode_duid(d) as u64, 8);
    b.extend_from_slice(&payload[..312]);
    b
}

/// The Phase 2 test signal: network status, a clear group call on channel 0, an encrypted one on channel 1 at the same time.
/// The clear call carries the test speech ([`crate::voice::test_speech_half`]), the encrypted one pseudo-random bits.
pub fn render(seed: u64) -> Vec<Complex32> {
    let mut rng = sdrkit_algo_sim::rng::Rng::new(seed);
    let mut rnd = |n: usize| -> Vec<u8> {
        let mut v = vec![0u8; n];
        rng.fill_bits(&mut v);
        v
    };
    let mut m = crate::dqpsk::DqpskModulator::new(6000.0, crate::front::RATE, 0.2);
    let mut out = Vec::new();
    crate::gen::silence(0.3, &mut out);
    let net = {
        let mut msg = vec![mac::ACTIVE << 5];
        let w: u64 = (0xBEE00u64 << 36) | (0x3A1 << 24) | (1 << 16) | (5 << 8);
        msg.extend_from_slice(&w.to_be_bytes()[1..]);
        msg.extend_from_slice(&0x293u16.to_be_bytes());
        msg
    };
    let ptt = |alg: u8, kid: u16, src: u32, tg: u16| {
        let mut msg = vec![mac::PTT << 5];
        msg.extend_from_slice(&[0u8; 9]);
        msg.push(alg);
        msg.extend_from_slice(&kid.to_be_bytes());
        msg.extend_from_slice(&src.to_be_bytes()[1..]);
        msg.extend_from_slice(&tg.to_be_bytes());
        msg
    };
    let end = vec![mac::END_PTT << 5];
    let calls: [(u8, u16, u32, u16); 2] = [(0x80, 0, 1234567, 1201), (0x84, 0x1234, 7654321, 1300)];
    let mut bits = Vec::new();
    let mut next_speech = 0;
    let total = 12 * 8;
    for i in 0..total {
        let loc = (i % 12) as u8;
        let ch = (loc & 1) as usize;
        let sync = loc == 5 || loc == 11;
        let (d, payload) = if i < 12 {
            (duid::FACCH, facch(&net))
        } else if i < 14 {
            let (a, k, sr, tg) = calls[ch];
            (duid::FACCH, facch(&ptt(a, k, sr, tg)))
        } else if i >= total - 2 {
            (duid::FACCH, facch(&end))
        } else if ch == 0 {
            let speech = crate::voice::test_speech_half();
            let mut p = Vec::with_capacity(312);
            for _ in 0..4 {
                p.extend_from_slice(&speech[next_speech % speech.len()]);
                next_speech += 1;
            }
            p.resize(312, 0);
            (duid::V4, p)
        } else {
            let mut p = rnd(288);
            p.resize(312, 0);
            (duid::V4, p)
        };
        bits.extend(slot_bits(if sync { 0 } else { loc }, sync, d, &payload));
    }
    m.bits(&bits, &mut out);
    m.flush(&mut out);
    crate::gen::silence(0.5, &mut out);
    out
}

/// The Phase 2 generator from the settings of the stack.
pub fn generator(cfg: &GenConfig) -> Prepared {
    Prepared::new(render(cfg.seed), cfg.rate_hz, true).with_info(12_500.0, 6000.0, 12_000.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duid_and_isch_round_trip() {
        for d in 0..16 {
            let w = encode_duid(d);
            assert_eq!(decode_duid(w), Some(d));
            assert_eq!(decode_duid(w ^ 0x10), Some(d));
        }
        let b = encode_isch(7, 1);
        assert_eq!(decode_isch(&b), Some((7, 1)));
    }
}
