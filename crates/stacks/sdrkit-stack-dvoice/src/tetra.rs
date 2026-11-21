//! TETRA downlink: π/4-DQPSK timeslots with the synchronization and normal bursts, the BSCH and SCH/HD channel coding, the
//! SYNC and SYSINFO broadcasts, call setup in MAC-RESOURCE, and the speech frames. Encrypted calls keep no identities.
//! Standard: ETSI EN 300 392-2.

use crate::conv::ConvChannel;
use crate::decoder::{Ctx, ModeParams, Protocol};
use crate::gen::Prepared;
use crate::items::*;
use crate::symbols::{Sym, SymAnswer, SyncAnswer, SyncPattern};
use crate::typed::TetraFields;
use sdrkit_algo_fec::conv::codes::{TETRA_2_3, TETRA_K5_R4};
use sdrkit_core::interface::Fields;
use sdrkit_core::media::Codec;
use sdrkit_core::publish::Published;
use sdrkit_core::publish::Declare;
use sdrkit_core::{Complex32, GenConfig, Value};
use sdrkit_util_bits::crc::catalog::{CRC16_GENIBUS, CRC8_SMBUS};
use sdrkit_util_bits::interleave::Permutation;
use sdrkit_util_bits::lfsr::Additive;
use sdrkit_util_bits::pack::{push_u64, to_u64};
use sdrkit_util_bits::seq;
use std::sync::LazyLock;

/// The sync training sequence (38 bits) and normal training sequences 1 and 2 (22 bits).
pub const SYNC_TRAINING: &str = "11000001100111001110100111000001100111";
pub const NORMAL_1: &str = "1101000011101001110100";
pub const NORMAL_2: &str = "0111101001000011011110";
/// The rate the symbols are read at.
pub const STREAM_RATE: f64 = 72_000.0;

/// The rate 2/3 RCPC code of the control channels (EN 300 392-2 clause 8.2.3.1): the rate 1/4 mother code punctured, for blocks
/// of up to 288 information bits with their CRC and tail.
pub fn rcpc() -> ConvChannel {
    ConvChannel::new(TETRA_K5_R4, Some(TETRA_2_3), 288 + 20)
}

/// The scrambling sequence of a colour code (30 bits: MCC, MNC, colour code. 0 for the BSCH), from p(1) (EN 300 392-2 clause
/// 8.2.5).
pub fn scrambling(colour: u32, out: &mut [u8]) {
    let mut l = seq::tetra(colour);
    l.fill_bits(out);
}

/// The block interleaves `(K, a)` of the BSCH, the SCH/HD and the SCH/F (EN 300 392-2 clause 8.2.4.1: bit i, counted from 1,
/// goes to 1 + (a i mod K)).
static ORDERS: LazyLock<[(usize, usize, Permutation); 3]> = LazyLock::new(|| {
    [(120, 11), (216, 101), (432, 103)].map(|(k, a)| {
        (
            k,
            a,
            Permutation::multiplicative(a, a, k).expect("a is prime to K"),
        )
    })
});

fn order(k: usize, a: usize) -> Option<&'static Permutation> {
    ORDERS
        .iter()
        .find(|(kk, aa, _)| *kk == k && *aa == a)
        .map(|(_, _, p)| p)
}

/// Codes a control block: information, CRC-16, 4 tail bits, RCPC 2/3, interleave `(K, a)`, scramble (EN 300 392-2 clause 8.2).
pub fn encode_block(info: &[u8], a: usize, colour: u32) -> Vec<u8> {
    let mut b = info.to_vec();
    let crc = CRC16_GENIBUS.bits(&b);
    push_u64(&mut b, crc as u64, 16);
    b.extend([0, 0, 0, 0]);
    let mut c = Vec::new();
    rcpc().encode(&b, &mut c);
    let mut out = vec![0u8; c.len()];
    match order(c.len(), a) {
        Some(p) if p.apply(&c, &mut out).is_ok() => {}
        _ => return Vec::new(),
    }
    Additive::new(seq::tetra(colour)).apply_bits(&mut out);
    out
}

/// Decodes a control block of `info` bits from its soft values (positive means 1): the information and the bits repaired, when
/// the CRC checks.
pub fn decode_block(
    conv: &mut ConvChannel,
    soft: &[f32],
    info: usize,
    a: usize,
    colour: u32,
) -> Option<(Vec<u8>, usize)> {
    let mut s = soft.to_vec();
    let mut pn = vec![0u8; s.len()];
    scrambling(colour, &mut pn);
    for (x, &b) in s.iter_mut().zip(&pn) {
        if b != 0 {
            *x = -*x;
        }
    }
    let mut d = vec![0f32; s.len()];
    order(s.len(), a)?.invert(&s, &mut d).ok()?;
    let (b, errs) = conv.decode(&d, info + 20, true).ok()?;
    CRC16_GENIBUS
        .verify_bits(&b[..info + 16])
        .then(|| (b[..info].to_vec(), errs))
}

/// The SYNC PDU of the BSCH (60 bits).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Sync {
    pub colour_code: u8,
    pub tn: u8,
    pub fn_: u8,
    pub mn: u8,
    pub mcc: u16,
    pub mnc: u16,
}

impl Sync {
    pub fn bits(&self) -> Vec<u8> {
        let mut b = Vec::with_capacity(60);
        push_u64(&mut b, 0, 4);
        push_u64(&mut b, self.colour_code as u64, 6);
        push_u64(&mut b, self.tn as u64, 2);
        push_u64(&mut b, self.fn_ as u64, 5);
        push_u64(&mut b, self.mn as u64, 6);
        push_u64(&mut b, 0, 8);
        push_u64(&mut b, self.mcc as u64, 10);
        push_u64(&mut b, self.mnc as u64, 14);
        push_u64(&mut b, 0, 5);
        b
    }
    pub fn from_bits(b: &[u8]) -> Sync {
        let f = |a: usize, n: usize| to_u64(&b[a..a + n]);
        Sync {
            colour_code: f(4, 6) as u8,
            tn: f(10, 2) as u8,
            fn_: f(12, 5) as u8,
            mn: f(17, 6) as u8,
            mcc: f(31, 10) as u16,
            mnc: f(41, 14) as u16,
        }
    }
    /// The 30-bit scrambling code.
    pub fn colour(&self) -> u32 {
        ((self.mcc as u32) << 20) | ((self.mnc as u32) << 6) | self.colour_code as u32
    }
}

const KEY: u32 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Burst {
    Sync,
    Normal(bool),
}

/// The TETRA protocol.
pub struct Tetra {
    buf: Vec<Sym>,
    burst: Option<Burst>,
    sync: Option<Sync>,
    site: SiteRecord,
    idle: usize,
    frames: u64,
    conv: ConvChannel,
}

impl Default for Tetra {
    fn default() -> Self {
        Tetra {
            buf: Vec::with_capacity(260),
            burst: None,
            sync: None,
            site: SiteRecord::default(),
            idle: 0,
            frames: 0,
            conv: rcpc(),
        }
    }
}

fn soft_bits(s: &[Sym]) -> Vec<f32> {
    let mut v = Vec::with_capacity(s.len() * 2);
    for x in s {
        let (a, b) = x.soft_dibit();
        v.push(a);
        v.push(b);
    }
    v
}

impl Tetra {
    fn sync_burst(&mut self, ctx: &mut Ctx) {
        let b = soft_bits(&self.buf);
        // BSCH at bits 92..212. SCH/HD at bits 280..496.
        let bsch = decode_block(&mut self.conv, &b[92..212], 60, 11, 0);
        ctx.identity(bsch.is_some());
        ctx.block(bsch.is_some(), 120, bsch.as_ref().map_or(0, |x| x.1));
        if let Some((info, _)) = bsch {
            let s = Sync::from_bits(&info);
            self.site.country = Some(u32::from(s.mcc));
            self.site.network = Some(u32::from(s.mnc));
            self.site.colour_code = Some(u32::from(s.colour_code));
            self.sync = Some(s);
        }
        let Some(s) = self.sync else {
            return;
        };
        let hd = decode_block(&mut self.conv, &b[280..496], 124, 101, s.colour());
        ctx.block(hd.is_some(), 216, hd.as_ref().map_or(0, |x| x.1));
        if let Some((info, _)) = hd {
            let f = |a: usize, n: usize| to_u64(&info[a..a + n]);
            if f(0, 2) == 2 && f(2, 2) == 0 {
                let carrier = f(4, 12);
                let band = f(16, 4);
                let offset = f(20, 2);
                let la = f(26, 14);
                let freq = band as f64 * 100e6
                    + carrier as f64 * 25e3
                    + [0.0, 6250.0, -6250.0, 12500.0][offset as usize];
                self.site.main_carrier = Some(carrier as u32);
                self.site.band = Some(band as u32);
                self.site.main_carrier_hz = Some(freq);
                self.site.location_area = Some(la as u32);
                self.site.valid = true;
                self.site.time = ctx.now;
                ctx.site(self.site.clone());
            }
        }
    }

    fn normal_burst(&mut self, traffic: bool, ctx: &mut Ctx) {
        let b = soft_bits(&self.buf);
        let Some(s) = self.sync else {
            return;
        };
        let blocks = [&b[12..228], &b[280..496]];
        if traffic {
            if !ctx.has_call(KEY) {
                return;
            }
            ctx.update_call(KEY, |c| c.voice_frames += 2);
            for (i, blk) in blocks.iter().enumerate() {
                let hard: Vec<u8> = blk.iter().map(|&v| (v > 0.0) as u8).collect();
                let ok = CRC8_SMBUS.bits(&hard[..137]) as u64 == to_u64(&hard[137..145]);
                ctx.block(ok, 216, 0);
                let spec = crate::voice::FrameSpec {
                    codec: Codec::TetraAcelp,
                    packaging: sdrkit_graph::SpeechPackaging::Tchs137,
                    codec_mode: None,
                    duration_ms: 30,
                    stack: "tetra",
                    carrier_hz: ctx.carrier_hz,
                    slot: s.tn + 1,
                };
                ctx.speech(
                    KEY,
                    spec,
                    &hard[..137],
                    ctx.now + i as f64 * 0.03,
                    !ok,
                    None,
                );
            }
        } else {
            // SCH/F: 268 bits over both blocks.
            let mut joined = blocks[0].to_vec();
            joined.extend_from_slice(blocks[1]);
            let r = decode_block(&mut self.conv, &joined, 268, 103, s.colour());
            ctx.identity(r.is_some());
            ctx.block(r.is_some(), 432, r.as_ref().map_or(0, |x| x.1));
            if let Some((info, _)) = r {
                self.mac(&info, ctx);
            }
        }
    }

    /// MAC-RESOURCE (type 0): encryption mode, address. Carrying a call control PDU (D-SETUP 7, D-RELEASE 6).
    fn mac(&mut self, m: &[u8], ctx: &mut Ctx) {
        let f = |a: usize, n: usize| to_u64(&m[a..a + n]);
        if f(0, 2) != 0 {
            return;
        }
        let enc_mode = f(4, 2) as u8;
        let ssi = f(16, 24) as u32;
        let pdu = f(40, 5);
        match pdu {
            7 => {
                let call_id = f(45, 14);
                let calling = f(59, 24);
                let group = f(83, 24);
                if ctx.has_call(KEY) {
                    return;
                }
                let encrypted = enc_mode != 0;
                let (src, dst) = if encrypted {
                    (Ident::None, Ident::None)
                } else {
                    (Ident::Number(calling), Ident::Number(group))
                };
                let enc = if encrypted {
                    Encryption::encrypted(Some(enc_mode as u32), "TEA", None)
                } else {
                    Encryption::clear()
                };
                let mut rec = CallRecord {
                    kind: CallKind::GroupVoice,
                    src,
                    dst,
                    group: true,
                    encryption: enc.clone(),
                    ..Default::default()
                };
                if !encrypted {
                    rec.set(CallDetail::CallId, Value::I64(call_id as i64));
                    rec.set(CallDetail::Address, Value::I64(ssi as i64));
                }
                if let Some(s) = self.sync {
                    rec.set(CallDetail::Mcc, Value::I64(i64::from(s.mcc)));
                    rec.set(CallDetail::Mnc, Value::I64(i64::from(s.mnc)));
                }
                ctx.start_call(KEY, rec, false);
                ctx.set_encryption(KEY, enc);
                ctx.control(ControlRecord {
                    opcode: 7,
                    name: "D-SETUP".into(),
                    text: if encrypted {
                        "call setup (encrypted)".into()
                    } else {
                        format!("call setup {calling} -> {group}")
                    },
                    time: ctx.now,
                    ..Default::default()
                });
            }
            6 if ctx.has_call(KEY) => ctx.end_call(KEY, true),
            _ => {}
        }
    }
}

impl Protocol for Tetra {
    fn params(&self) -> ModeParams {
        ModeParams {
            mode: "tetra",
            name: "TETRA",
            baud: 18_000.0,
            two_level: false,
            channel_hz: 25_000.0,
            calls: &crate::results::CALLS_300,
            message_cap: 100,
            polarity_rule: false,
            clock_loop: false,
            extra_baud: None,
            stream_rate: STREAM_RATE,
        }
    }

    fn patterns(&self) -> Vec<SyncPattern> {
        // The training sequences, as phase steps.
        let mk = |id: u8, s: &str, before: usize| {
            let mut p = SyncPattern::from_hex_dibits(id, "0", before);
            p.symbols = s
                .as_bytes()
                .chunks(2)
                .map(|c| {
                    sdrkit_algo_fec::conv::tcm::dibit_symbol(((c[0] - b'0') << 1) | (c[1] - b'0'))
                })
                .collect();
            p.max_err = p.symbols.len() / 8;
            p.no_auto_invert()
        };
        vec![
            mk(0, SYNC_TRAINING, 106),
            mk(1, NORMAL_1, 121),
            mk(2, NORMAL_2, 121),
        ]
    }

    fn on_sync(
        &mut self,
        id: u8,
        _inv: bool,
        pre: &[Sym],
        _at: u64,
        tracking: bool,
        _ctx: &mut Ctx,
    ) -> SyncAnswer {
        if tracking && self.burst.is_some() && self.buf.len() > 5 && self.buf.len() < 240 {
            return SyncAnswer::Ignore;
        }
        self.burst = Some(match id {
            0 => Burst::Sync,
            1 => Burst::Normal(true),
            _ => Burst::Normal(false),
        });
        self.buf.clear();
        self.buf.extend_from_slice(pre);
        self.idle = 0;
        SyncAnswer::Take
    }

    fn on_symbol(&mut self, s: Sym, _at: u64, ctx: &mut Ctx) -> SymAnswer {
        let Some(b) = self.burst else {
            self.idle += 1;
            return if self.idle > 4000 {
                SymAnswer::Stop
            } else {
                SymAnswer::Continue
            };
        };
        self.buf.push(s);
        if self.buf.len() == 255 {
            if crate::symbols::carrier_gone(&self.buf) {
                self.burst = None;
                if ctx.has_call(KEY) {
                    ctx.end_call(KEY, false);
                }
                return SymAnswer::Stop;
            }
            self.frames += 1;
            match b {
                Burst::Sync => self.sync_burst(ctx),
                Burst::Normal(t) => self.normal_burst(t, ctx),
            }
            self.burst = None;
        }
        SymAnswer::Continue
    }

    fn on_tick(&mut self, ctx: &mut Ctx) {
        if ctx.has_call(KEY) && ctx.now - ctx.last_frame_s > 1.0 {
            ctx.end_call(KEY, false);
        }
    }

    fn fields(&self) -> Vec<&'static dyn Declare> {
        <TetraFields as Fields>::SHAPE.fields.to_vec()
    }

    fn publish(&self, r: &mut Published) {
        r.put(&TetraFields {
            bursts: self.frames as i64,
        });
    }

    fn codec(&self) -> Codec {
        Codec::TetraAcelp
    }
}

// ---------------------------------------------------------------------------------------------------------------------------
// Generator.

fn bits(s: &str) -> Vec<u8> {
    s.bytes().map(|c| c - b'0').collect()
}

fn sync_burst(s: &Sync, sysinfo: &[u8]) -> Vec<u8> {
    let mut b = vec![0u8; 12];
    b.extend([1u8; 8]);
    b.extend([0u8; 64]);
    b.extend([1u8; 8]);
    b.extend(encode_block(&s.bits(), 11, 0));
    b.extend(bits(SYNC_TRAINING));
    b.extend([0u8; 30]);
    b.extend(encode_block(sysinfo, 101, s.colour()));
    b.extend([0u8; 14]);
    debug_assert_eq!(b.len(), 510);
    b
}

fn normal_burst(b1: &[u8], b2: &[u8], traffic: bool) -> Vec<u8> {
    let mut b = vec![0u8; 12];
    b.extend_from_slice(b1);
    b.extend([0u8; 14]);
    b.extend(bits(if traffic { NORMAL_1 } else { NORMAL_2 }));
    b.extend([0u8; 16]);
    b.extend_from_slice(b2);
    b.extend([0u8; 14]);
    b
}

fn speech_block(rng: &mut sdrkit_algo_sim::rng::Rng) -> Vec<u8> {
    let mut v = Vec::with_capacity(216);
    for _ in 0..137 {
        v.push(rng.bit());
    }
    let crc = CRC8_SMBUS.bits(&v);
    push_u64(&mut v, crc as u64, 8);
    let rep: Vec<u8> = v[..71].to_vec();
    v.extend(rep);
    v
}

fn mac_resource(enc: u8, pdu: u64, call_id: u64, calling: u64, group: u64) -> Vec<u8> {
    let mut m = Vec::with_capacity(268);
    push_u64(&mut m, 0, 2);
    push_u64(&mut m, 0, 2);
    push_u64(&mut m, enc as u64, 2);
    push_u64(&mut m, 0, 10);
    push_u64(&mut m, group, 24);
    push_u64(&mut m, pdu, 5);
    push_u64(&mut m, call_id, 14);
    push_u64(&mut m, calling, 24);
    push_u64(&mut m, group, 24);
    m.resize(268, 0);
    m
}

/// The TETRA test signal: a cell (MCC 234, MNC 10, colour code 1) with a clear group call and an encrypted one.
pub fn render(seed: u64) -> Vec<Complex32> {
    let mut rng = sdrkit_algo_sim::rng::Rng::new(seed);
    let sync = Sync {
        colour_code: 1,
        tn: 0,
        fn_: 18,
        mn: 1,
        mcc: 234,
        mnc: 10,
    };
    let mut sysinfo = Vec::with_capacity(124);
    push_u64(&mut sysinfo, 2, 2);
    push_u64(&mut sysinfo, 0, 2);
    push_u64(&mut sysinfo, 1400, 12);
    push_u64(&mut sysinfo, 3, 4);
    push_u64(&mut sysinfo, 0, 2);
    push_u64(&mut sysinfo, 0, 4);
    push_u64(&mut sysinfo, 1001, 14);
    sysinfo.resize(124, 0);
    let mut m = crate::dqpsk::DqpskModulator::new(18_000.0, STREAM_RATE, 0.35);
    let mut out = Vec::new();
    out.extend(std::iter::repeat_n(
        Complex32::new(0.0, 0.0),
        (0.2 * STREAM_RATE) as usize,
    ));
    let mut b = Vec::new();
    for (enc, calling, group) in [(0u8, 2_001_234u64, 9001u64), (1, 2_001_999, 9002)] {
        for _ in 0..4 {
            b.extend(sync_burst(&sync, &sysinfo));
        }
        let setup = encode_block(
            &mac_resource(enc, 7, 77, calling, group),
            103,
            sync.colour(),
        );
        b.extend(normal_burst(&setup[..216], &setup[216..], false));
        for k in 0..60 {
            if k % 18 == 17 {
                b.extend(sync_burst(&sync, &sysinfo));
            } else {
                let (x, y) = (speech_block(&mut rng), speech_block(&mut rng));
                b.extend(normal_burst(&x, &y, true));
            }
        }
        let rel = encode_block(
            &mac_resource(enc, 6, 77, calling, group),
            103,
            sync.colour(),
        );
        b.extend(normal_burst(&rel[..216], &rel[216..], false));
    }
    m.bits(&b, &mut out);
    m.flush(&mut out);
    out.extend(std::iter::repeat_n(
        Complex32::new(0.0, 0.0),
        (0.3 * STREAM_RATE) as usize,
    ));
    out
}

/// The TETRA generator from the settings of the stack.
pub fn generator(cfg: &GenConfig) -> Prepared {
    Prepared::with_source_rate(render(cfg.seed), STREAM_RATE, cfg.rate_hz, true)
        .with_info(25_000.0, 18_000.0, 36_000.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn control_blocks_round_trip() {
        let s = Sync {
            colour_code: 5,
            tn: 2,
            fn_: 18,
            mn: 33,
            mcc: 234,
            mnc: 10,
        };
        let c = encode_block(&s.bits(), 11, 0);
        assert_eq!(c.len(), 120);
        let mut soft: Vec<f32> = c.iter().map(|&b| if b != 0 { 1.0 } else { -1.0 }).collect();
        soft[30] = -soft[30];
        let (info, _) = decode_block(&mut rcpc(), &soft, 60, 11, 0).unwrap();
        assert_eq!(Sync::from_bits(&info), s);
        let hd: Vec<u8> = (0..124).map(|i| (i % 5 == 1) as u8).collect();
        let c = encode_block(&hd, 101, s.colour());
        assert_eq!(c.len(), 216);
        let soft: Vec<f32> = c.iter().map(|&b| if b != 0 { 1.0 } else { -1.0 }).collect();
        assert_eq!(
            decode_block(&mut rcpc(), &soft, 124, 101, s.colour())
                .unwrap()
                .0,
            hd
        );
        let f = vec![1u8; 268];
        assert_eq!(encode_block(&f, 103, 7).len(), 432);
    }

    #[test]
    fn the_scrambling_sequence_starts_at_p1_not_with_its_seed() {
        // EN 300 392-2 clause 8.2.5: p(k) for k = -31 .. 0 are the seed (two ones, then the colour code). The sequence sent
        // starts at p(1), the first bit the register makes.
        let colour = (234 << 20) | (10 << 6) | 1;
        let mut got = [0u8; 64];
        scrambling(colour, &mut got);
        let seed: Vec<u8> = (0..32)
            .map(|i| ((((colour << 2) | 3) >> (31 - i)) & 1) as u8)
            .collect();
        assert_ne!(got[..32], seed[..]);
        // The control blocks are scrambled with it.
        let c = encode_block(&[0u8; 60], 11, colour);
        let plain = encode_block(&[0u8; 60], 11, 0);
        let mut bsch = [0u8; 120];
        scrambling(0, &mut bsch);
        let mut cell = [0u8; 120];
        scrambling(colour, &mut cell);
        for i in 0..120 {
            assert_eq!(c[i] ^ cell[i], plain[i] ^ bsch[i]);
        }
    }
}
