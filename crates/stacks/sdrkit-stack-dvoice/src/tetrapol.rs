//! TETRAPOL: GMSK frames of 160 bits, the signalling frames (system identity, call setup and release) and the voice frames,
//! whose 120 codec bits leave as speech frames. Encrypted calls keep no identities.
//! Standard: PAS 0001-2, PAS 0001-3.

use crate::conv::ConvChannel;
use crate::decoder::{Ctx, ModeParams, Protocol};
use crate::gen::{silence, FskModulator, Prepared, TxPulse};
use crate::items::*;
use crate::symbols::{Sym, SymAnswer, SyncAnswer, SyncPattern};
use crate::typed::TetrapolFields;
use sdrkit_algo_fec::conv::codes::DSTAR_K3;
use sdrkit_algo_modem::gmsk::{discriminator_taps, Mlse};
use sdrkit_core::interface::Fields;
use sdrkit_core::media::Codec;
use sdrkit_core::publish::Declare;
use sdrkit_core::publish::Published;
use sdrkit_core::{Complex32, GenConfig};
use sdrkit_util_bits::crc::catalog::CRC16_GSM;
use sdrkit_util_bits::pack::{push_u64, to_u64};

/// Frame syncs: signalling and voice.
pub const SYNC_SIG: &str = "1110011011010000";
pub const SYNC_VOICE: &str = "0001100100101111";

/// The bandwidth-time product of the GMSK of TETRAPOL.
pub const BT: f64 = 0.25;
/// Symbols after a sync: a signalling or a voice frame.
const FRAME: usize = 144;
/// Sync symbols kept for the sequence estimator.
const SYNC_LEN: usize = 16;
/// Frames read on after a frame whose next sync the symbol layer did not find. The two syncs are each other's complement, so
/// the sign of the mean of the 16 values says which one it is.
const MAX_COAST: u32 = 16;
/// The mean of the sync values against the signalling sync that names a sync without the symbol layer (1 for a clean sync).
const COAST_LEVEL: f32 = 0.6;

/// The bits of a frame from its symbol values: the integrate-and-dump of the discriminator leaves the intersymbol interference
/// of the Gaussian pulse ([`discriminator_taps`]), which a sequence estimator removes. The scale of the centre tap and the
/// noise come from the sync before the frame, whose bits are known; the last bits of the sync are the start of the trellis.
struct SequenceEstimator {
    taps: Vec<f32>,
    mlse: Mlse,
    obs: Vec<Complex32>,
    llr: Vec<f32>,
    /// The values of the last sync and its levels (+1 for bit 1).
    sync_v: [f32; SYNC_LEN],
    sync_p: [f32; SYNC_LEN],
}

impl SequenceEstimator {
    fn new() -> Self {
        let taps = discriminator_taps(BT).expect("a positive bandwidth-time product");
        let g: Vec<Complex32> = taps.iter().map(|&t| Complex32::new(t, 0.0)).collect();
        SequenceEstimator {
            mlse: Mlse::new(&g, FRAME).expect("a short channel"),
            taps,
            obs: Vec::with_capacity(FRAME),
            llr: vec![0.0; FRAME],
            sync_v: [0.0; SYNC_LEN],
            sync_p: [0.0; SYNC_LEN],
        }
    }

    /// Keeps the sync `pre` (its last 16 values) of the pattern `bits`.
    fn sync(&mut self, pre: &[Sym], bits: &str) {
        let at = pre.len().saturating_sub(SYNC_LEN);
        for (k, (s, b)) in pre[at..].iter().zip(bits.bytes()).enumerate() {
            self.sync_v[k] = s.v;
            self.sync_p[k] = if b == b'1' { 1.0 } else { -1.0 };
        }
    }

    /// The level of sync symbol `k` that the channel gives for the known bits.
    fn model(&self, k: usize) -> f32 {
        let c = self.taps.len() / 2;
        (0..self.taps.len())
            .map(|m| self.taps[m] * self.sync_p[k + c - m])
            .sum()
    }

    /// Soft values of the frame `buf` (positive means 1, about unit size), into `out`. False when the sync gives no level.
    fn frame(&mut self, buf: &[Sym], out: &mut Vec<f32>) -> bool {
        let c = self.taps.len() / 2;
        let inner = c..SYNC_LEN - c;
        let (mut num, mut den) = (0f32, 0f32);
        for k in inner.clone() {
            let m = self.model(k);
            num += self.sync_v[k] * m;
            den += m * m;
        }
        let scale = num / den;
        if !(scale > 0.0 && scale.is_finite()) {
            return false;
        }
        let resid = inner
            .clone()
            .map(|k| (self.sync_v[k] / scale - self.model(k)).powi(2))
            .sum::<f32>()
            / inner.len() as f32;
        let noise = resid.clamp(0.02, 1.0);
        // Observation j has frame symbol j as its newest symbol: the last c sync values come first, and the last 2c sync bits
        // are the start state (the newest in the top bit; bit 0 of the estimator is level +1). The last c frame symbols have
        // no observation after them.
        self.obs.clear();
        for k in SYNC_LEN - c..SYNC_LEN {
            self.obs.push(Complex32::new(self.sync_v[k] / scale, 0.0));
        }
        self.obs.extend(
            buf[..FRAME - c]
                .iter()
                .map(|s| Complex32::new(s.v / scale, 0.0)),
        );
        let start = (0..2 * c).fold(0u32, |st, i| {
            let p = self.sync_p[SYNC_LEN - 1 - i];
            st | (u32::from(p < 0.0) << (2 * c - 1 - i))
        });
        let n = self.obs.len();
        if self
            .mlse
            .soft(&self.obs, noise, Some(start), &mut self.llr[..n])
            .is_err()
        {
            return false;
        }
        out.clear();
        out.extend(
            self.llr[..n]
                .iter()
                .map(|&l| (l * noise / 2.0).clamp(-1.5, 1.5)),
        );
        true
    }
}

/// The convolutional code of the signalling frames (rate 1/2, constraint 3), 72 input bits with the tail.
pub fn channel() -> ConvChannel {
    ConvChannel::new(DSTAR_K3, None, 72)
}

/// Signalling: 54 bits + CRC-16 + 2 tail bits, rate 1/2: 144 bits.
pub fn encode_sig(info: &[u8]) -> Vec<u8> {
    let mut b = info[..54].to_vec();
    let crc = CRC16_GSM.bits(&b);
    push_u64(&mut b, crc as u64, 16);
    b.extend([0, 0]);
    let mut out = Vec::with_capacity(144);
    channel().encode(&b, &mut out);
    out
}

/// Decodes a signalling frame from its 144 soft values (positive means 1): the 54 information bits and the bits repaired, when
/// the CRC checks.
pub fn decode_sig(conv: &mut ConvChannel, soft: &[f32]) -> Option<(Vec<u8>, usize)> {
    let (b, errs) = conv.decode(soft, 72, true).ok()?;
    CRC16_GSM
        .verify_bits(&b[..70])
        .then(|| (b[..54].to_vec(), errs))
}

/// Message types of the signalling frames.
pub mod msg {
    pub const SYSTEM: u64 = 1;
    pub const CALL_SETUP: u64 = 2;
    pub const CALL_END: u64 = 3;
}

/// The TETRAPOL protocol.
pub struct Tetrapol {
    buf: Vec<Sym>,
    kind: Option<u8>,
    idle: usize,
    site: SiteRecord,
    voice_frames: u64,
    signalling_frames: u64,
    conv: ConvChannel,
    sequence: SequenceEstimator,
    soft: Vec<f32>,
    /// A frame just ended: the next 16 symbols are the next sync.
    coast: bool,
    coasted: u32,
    sync_buf: Vec<Sym>,
}

impl Default for Tetrapol {
    fn default() -> Self {
        Tetrapol {
            buf: Vec::with_capacity(160),
            kind: None,
            idle: 0,
            site: SiteRecord::default(),
            voice_frames: 0,
            signalling_frames: 0,
            conv: channel(),
            sequence: SequenceEstimator::new(),
            soft: Vec::with_capacity(FRAME),
            coast: false,
            coasted: 0,
            sync_buf: Vec::with_capacity(SYNC_LEN),
        }
    }
}

const KEY: u32 = 1;

impl Tetrapol {
    fn frame(&mut self, kind: u8, ctx: &mut Ctx) {
        let mut soft = std::mem::take(&mut self.soft);
        if !self.sequence.frame(&self.buf, &mut soft) {
            soft.clear();
            soft.extend(self.buf.iter().map(|s| s.v.clamp(-1.5, 1.5)));
        }
        self.frame_bits(kind, &soft, ctx);
        self.soft = soft;
    }

    /// The 16 symbols after a frame, when the symbol layer found no sync in them: the frame that follows is read when they
    /// match one of the syncs (frames follow each other without a gap).
    fn coast_sync(&mut self) {
        if crate::symbols::carrier_gone(&self.sync_buf) {
            return;
        }
        let mean = self
            .sync_buf
            .iter()
            .zip(SYNC_SIG.bytes())
            .map(|(s, b)| s.v * if b == b'1' { 1.0 } else { -1.0 })
            .sum::<f32>()
            / SYNC_LEN as f32;
        let id = if mean >= COAST_LEVEL {
            0
        } else if mean <= -COAST_LEVEL {
            1
        } else {
            return;
        };
        let sync_buf = std::mem::take(&mut self.sync_buf);
        self.sequence
            .sync(&sync_buf, if id == 0 { SYNC_SIG } else { SYNC_VOICE });
        self.sync_buf = sync_buf;
        self.coasted += 1;
        self.kind = Some(id);
        self.buf.clear();
        self.idle = 0;
    }

    fn frame_bits(&mut self, kind: u8, soft: &[f32], ctx: &mut Ctx) {
        if kind == 0 {
            let r = decode_sig(&mut self.conv, soft);
            ctx.identity(r.is_some());
            ctx.block(r.is_some(), 144, r.as_ref().map_or(0, |x| x.1));
            let Some((b, _)) = r else {
                return;
            };
            self.signalling_frames += 1;
            let f = |a: usize, n: usize| to_u64(&b[a..a + n]);
            match f(0, 4) {
                msg::SYSTEM => {
                    self.site.country = Some(f(4, 10) as u32);
                    self.site.network = Some(f(14, 8) as u32);
                    self.site.base_station = Some(f(22, 12) as u32);
                    self.site.channel = Some(f(34, 12) as u32);
                    self.site.valid = true;
                    self.site.time = ctx.now;
                    ctx.site(self.site.clone());
                }
                msg::CALL_SETUP => {
                    let encrypted = b[4] != 0;
                    let group = b[5] != 0;
                    if ctx.has_call(KEY) {
                        return;
                    }
                    // An encrypted call keeps no identities.
                    let (src, dst) = if encrypted {
                        (Ident::None, Ident::None)
                    } else {
                        (Ident::Number(f(6, 24)), Ident::Number(f(30, 24)))
                    };
                    let enc = if encrypted {
                        Encryption::encrypted(None, "TETRAPOL", None)
                    } else {
                        Encryption::clear()
                    };
                    let rec = CallRecord {
                        kind: CallKind::Voice,
                        src,
                        dst,
                        group,
                        encryption: enc.clone(),
                        ..Default::default()
                    };
                    ctx.start_call(KEY, rec, false);
                    ctx.set_encryption(KEY, enc);
                }
                msg::CALL_END if ctx.has_call(KEY) => ctx.end_call(KEY, true),
                _ => {}
            }
        } else {
            let hard: Vec<u8> = soft.iter().map(|&v| (v > 0.0) as u8).collect();
            let ok = CRC16_GSM.bits(&hard[..20]) as u64 == to_u64(&hard[120..136]);
            ctx.identity(true);
            ctx.block(ok, 136, 0);
            self.voice_frames += 1;
            if ctx.has_call(KEY) {
                ctx.update_call(KEY, |c| c.voice_frames += 1);
                // tp120: bit k in byte k/8 at mask 0x80 >> (k mod 8), then a flag byte (bit 0: damaged).
                let mut data = [0u8; 16];
                for (k, &b) in hard[..120].iter().enumerate() {
                    data[k / 8] |= b << (7 - k % 8);
                }
                data[15] = (!ok) as u8;
                let spec = crate::voice::FrameSpec {
                    codec: Codec::TetrapolRpcelp,
                    packaging: sdrkit_graph::SpeechPackaging::Tp120,
                    codec_mode: None,
                    duration_ms: 20,
                    stack: "tetrapol",
                    carrier_hz: ctx.carrier_hz,
                    slot: 0,
                };
                ctx.speech(KEY, spec, &data, ctx.now, !ok, None);
            }
        }
    }
}

impl Protocol for Tetrapol {
    fn params(&self) -> ModeParams {
        ModeParams {
            mode: "tetrapol",
            name: "TETRAPOL",
            baud: 8000.0,
            two_level: true,
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
        // The two syncs are each other's complement: polarity is not taken from them.
        vec![
            SyncPattern::from_bits(0, SYNC_SIG, 0)
                .with_max_err(1)
                .no_auto_invert(),
            SyncPattern::from_bits(1, SYNC_VOICE, 0)
                .with_max_err(1)
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
        _ctx: &mut Ctx,
    ) -> SyncAnswer {
        if tracking && self.kind.is_some() && self.buf.len() > 4 && self.buf.len() < 140 {
            return SyncAnswer::Ignore;
        }
        self.sequence
            .sync(pre, if id == 0 { SYNC_SIG } else { SYNC_VOICE });
        (self.coast, self.coasted) = (false, 0);
        self.kind = Some(id);
        self.buf.clear();
        self.idle = 0;
        SyncAnswer::Take
    }

    fn on_symbol(&mut self, s: Sym, _at: u64, ctx: &mut Ctx) -> SymAnswer {
        if self.kind.is_none() && self.coast {
            self.sync_buf.push(s);
            if self.sync_buf.len() == SYNC_LEN {
                self.coast = false;
                self.coast_sync();
            }
            return SymAnswer::Continue;
        }
        let Some(kind) = self.kind else {
            self.idle += 1;
            return if self.idle > 2000 {
                SymAnswer::Stop
            } else {
                SymAnswer::Continue
            };
        };
        self.buf.push(s);
        if self.buf.len() == FRAME {
            if crate::symbols::carrier_gone(&self.buf) {
                self.kind = None;
                return SymAnswer::Stop;
            }
            self.frame(kind, ctx);
            self.kind = None;
            self.coast = self.coasted < MAX_COAST;
            self.sync_buf.clear();
        }
        SymAnswer::Continue
    }

    fn on_tick(&mut self, ctx: &mut Ctx) {
        if ctx.has_call(KEY) && ctx.now - ctx.last_frame_s > 1.0 {
            ctx.end_call(KEY, false);
        }
    }

    fn fields(&self) -> Vec<&'static dyn Declare> {
        <TetrapolFields as Fields>::SHAPE.fields.to_vec()
    }

    fn publish(&self, r: &mut Published) {
        r.put(&TetrapolFields {
            tetrapol_voice_frames: self.voice_frames as i64,
            tetrapol_signalling_frames: self.signalling_frames as i64,
        });
    }

    fn codec(&self) -> Codec {
        Codec::TetrapolRpcelp
    }
}

// ---------------------------------------------------------------------------------------------------------------------------
// Generator.

fn sync(s: &str) -> Vec<u8> {
    s.bytes().map(|c| c - b'0').collect()
}

fn sig(fields: &[(u64, usize)]) -> Vec<u8> {
    let mut b = Vec::with_capacity(160);
    b.extend(sync(SYNC_SIG));
    let mut info = Vec::with_capacity(54);
    for &(v, n) in fields {
        push_u64(&mut info, v, n as u32);
    }
    info.resize(54, 0);
    b.extend(encode_sig(&info));
    b
}

fn voice(rng: &mut sdrkit_algo_sim::rng::Rng) -> Vec<u8> {
    let mut b = sync(SYNC_VOICE);
    let mut v = vec![0u8; 120];
    rng.fill_bits(&mut v);
    let crc = CRC16_GSM.bits(&v[..20]);
    b.extend(&v);
    push_u64(&mut b, crc as u64, 16);
    b.extend([0u8; 8]);
    b
}

/// The TETRAPOL test signal: system identity, a clear group call, an encrypted call.
pub fn render(seed: u64) -> Vec<Complex32> {
    let mut rng = sdrkit_algo_sim::rng::Rng::new(seed);
    let mut m = FskModulator::new(8000.0, TxPulse::Gaussian(0.25), 2000.0);
    let mut out = Vec::new();
    silence(0.3, &mut out);
    for (enc, src, dst) in [(false, 1201u64, 3001u64), (true, 1305, 3002)] {
        let mut bits = Vec::new();
        for _ in 0..3 {
            bits.extend(sig(&[
                (msg::SYSTEM, 4),
                (208, 10),
                (12, 8),
                (345, 12),
                (77, 12),
            ]));
        }
        bits.extend(sig(&[
            (msg::CALL_SETUP, 4),
            (enc as u64, 1),
            (1, 1),
            (src, 24),
            (dst, 24),
        ]));
        for _ in 0..60 {
            bits.extend(voice(&mut rng));
        }
        bits.extend(sig(&[(msg::CALL_END, 4)]));
        let lv: Vec<f32> = bits
            .iter()
            .map(|&b| if b != 0 { 1.0 } else { -1.0 })
            .collect();
        m.symbols(&lv, &mut out);
        m.flush(8, &mut out);
        m.reset();
        silence(0.5, &mut out);
    }
    out
}

/// The TETRAPOL generator from the settings of the stack.
pub fn generator(cfg: &GenConfig) -> Prepared {
    Prepared::new(render(cfg.seed), cfg.rate_hz, true).with_info(12_500.0, 8000.0, 8000.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signalling_round_trips() {
        let info: Vec<u8> = (0..54).map(|i| (i % 3 == 0) as u8).collect();
        let mut s: Vec<f32> = encode_sig(&info)
            .iter()
            .map(|&b| if b != 0 { 1.0 } else { -1.0 })
            .collect();
        s[9] = -s[9];
        assert_eq!(decode_sig(&mut channel(), &s).unwrap().0, info);
    }
}
