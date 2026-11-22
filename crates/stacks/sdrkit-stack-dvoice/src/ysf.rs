//! Yaesu System Fusion: the frame information channel, the callsigns of header and terminator frames, the communication frames
//! of every data type, the text and GPS of the data channel, and the voice frames.
//! Standard: Yaesu Amateur Radio Digital Specs 1.01.

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
use sdrkit_core::publish::Published;
use sdrkit_core::publish::Declare;
use sdrkit_core::{Complex32, GenConfig, Value};
use sdrkit_util_bits::crc::catalog::CRC16_GSM;
use sdrkit_util_bits::interleave::{Permutation, ReadOrder};
use sdrkit_util_bits::pack::{push_u64, to_u64};
use std::sync::LazyLock;

pub const FSW: &str = "D471C9634D";

/// The frame information channel.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Fich {
    /// 0 header, 1 communication, 2 terminator, 3 test.
    pub fi: u8,
    pub cs: u8,
    /// 0 group CQ, 3 individual.
    pub cm: u8,
    pub bn: u8,
    pub bt: u8,
    pub fn_: u8,
    pub ft: u8,
    /// Data type: 0 V/D mode 1, 1 data FR, 2 V/D mode 2, 3 voice FR.
    pub dt: u8,
    pub mr: u8,
    pub sql: bool,
    pub sc: u8,
}

impl Fich {
    pub fn to_bits(&self) -> Vec<u8> {
        let mut b = Vec::with_capacity(32);
        push_u64(&mut b, self.fi as u64, 2);
        push_u64(&mut b, self.cs as u64, 2);
        push_u64(&mut b, self.cm as u64, 2);
        push_u64(&mut b, self.bn as u64, 2);
        push_u64(&mut b, self.bt as u64, 2);
        push_u64(&mut b, self.fn_ as u64, 3);
        push_u64(&mut b, self.ft as u64, 3);
        push_u64(&mut b, 0, 3);
        push_u64(&mut b, self.mr as u64, 3);
        push_u64(&mut b, 0, 1);
        push_u64(&mut b, self.dt as u64, 2);
        push_u64(&mut b, self.sql as u64, 1);
        push_u64(&mut b, self.sc as u64 & 0x3F, 6);
        b
    }
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
            mr: f(19, 3),
            dt: f(23, 2),
            sql: b[25] != 0,
            sc: f(26, 6),
        }
    }
}

/// Extended Golay (24,12) of the FICH words.
static FICH_WORD: LazyLock<SmallCode> = LazyLock::new(SmallCode::golay_24_12);
/// The 20 x 10 interleave of the FICH and the data channel: the coded bits are written in rows of 20 and read by columns.
static INTERLEAVE: LazyLock<Permutation> =
    LazyLock::new(|| Permutation::row_column(10, 20, ReadOrder::Columns).expect("a whole matrix"));

/// The convolutional decoder of the FICH and the data channel (100 input bits with the tail).
pub fn channel() -> ConvChannel {
    ConvChannel::new(NXDN_K5, None, 100)
}

fn interleave(c: &[u8]) -> Vec<u8> {
    let mut out = vec![0u8; 200];
    if c.len() != 200 || INTERLEAVE.apply(c, &mut out).is_err() {
        out.clear();
    }
    out
}

fn deinterleave(s: &[f32]) -> Option<[f32; 200]> {
    let mut out = [0f32; 200];
    INTERLEAVE.invert(s.get(..200)?, &mut out).ok()?;
    Some(out)
}

/// FICH: 32 bits + CRC-16, four Golay (24,12) words, 4 tail bits, rate 1/2, a 20 x 10 interleave: 200 bits.
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
    let c = deinterleave(soft)?;
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

/// A data channel block: 10 bytes + CRC-16, rate 1/2: 200 bits (sent as five pieces of 40 in V/D mode 2).
pub fn encode_dch(data: &[u8; 10]) -> Vec<u8> {
    let mut b = bytes_to_bits(data);
    let crc = CRC16_GSM.bits(&b);
    push_u64(&mut b, crc as u64, 16);
    b.extend([0, 0, 0, 0]);
    let mut c = Vec::with_capacity(200);
    channel().encode(&b, &mut c);
    interleave(&c)
}

/// Decodes the 200 soft values of a data channel block: the 10 bytes and the bits repaired, when the CRC checks.
pub fn decode_dch(conv: &mut ConvChannel, soft: &[f32]) -> Option<([u8; 10], usize)> {
    let c = deinterleave(soft)?;
    let (b, errs) = conv.decode(&c, 100, true).ok()?;
    if !CRC16_GSM.verify_bits(&b[..96]) {
        return None;
    }
    Some((bits_to_bytes(&b[..80]).try_into().ok()?, errs))
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

/// The System Fusion protocol.
pub struct Ysf {
    buf: Vec<Sym>,
    active: bool,
    idle: usize,
    call: Call,
    conv: ConvChannel,
}

impl Default for Ysf {
    fn default() -> Self {
        Ysf {
            buf: Vec::with_capacity(480),
            active: false,
            idle: 0,
            call: Call::default(),
            conv: channel(),
        }
    }
}

const KEY: u32 = 1;

/// The data type of a communication frame of full-rate data: no voice.
const DT_DATA_FR: u8 = 1;
/// The data type of V/D mode 2.
const DT_VD2: u8 = 2;

/// The codec and packaging of the speech frames of a communication frame of data type `dt`, when the stack sends them.
/// Only V/D mode 2 (`amb2`, `info49`). V/D mode 1 (`amb2` `codewords`) and voice FR (`imbe`) are not sent: the place of
/// their bits in the frame is not known. Data FR has no voice.
fn speech_of(dt: u8) -> Option<(Codec, sdrkit_graph::SpeechPackaging)> {
    (dt == DT_VD2).then_some((Codec::Ambe2, sdrkit_graph::SpeechPackaging::Info49))
}

fn dt_name(dt: u8) -> &'static str {
    match dt {
        0 => "V/D mode 1",
        1 => "data FR",
        2 => "V/D mode 2",
        _ => "voice FR",
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
            return;
        };
        let payload = &soft[200..920];
        // The data channel: five pieces of 40 bits, one at the start of each 144-bit section.
        let mut dch = Vec::with_capacity(200);
        for k in 0..5 {
            dch.extend_from_slice(&payload[k * 144..k * 144 + 40]);
        }
        let d = decode_dch(&mut self.conv, &dch);
        ctx.block(d.is_some(), 200, d.map_or(0, |x| x.1));
        match fich.fi {
            0 => {
                // Header: CSD1 (destination and source, 10 characters each) over two data blocks.
                if let Some((b, _)) = d {
                    let text = String::from_utf8_lossy(&b).trim_end().to_string();
                    match fich.fn_ {
                        0 => self.call.dest = text,
                        1 => self.call.src = text,
                        2 => self.call.down = text,
                        _ => self.call.up = text,
                    }
                }
                if fich.fn_ == 3 && !self.call.src.is_empty() && !ctx.has_call(KEY) {
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
                if fich.dt != DT_DATA_FR {
                    ctx.update_call(KEY, |c| c.voice_frames += 5);
                }
                if let Some((codec, packaging)) = speech_of(fich.dt) {
                    for k in 0..5 {
                        let sec: Vec<u8> = payload[k * 144 + 40..k * 144 + 144]
                            .iter()
                            .map(|&v| (v > 0.0) as u8)
                            .collect();
                        // V/D mode 2: 49 bits sent with their 4-bit repetition and parity in 104 bits. The first 49 are the information.
                        let mut info = [0u8; 7];
                        for (i, &b) in sec.iter().take(49).enumerate() {
                            info[i / 8] |= b << (7 - i % 8);
                        }
                        let spec = crate::voice::FrameSpec {
                            codec,
                            packaging,
                            codec_mode: None,
                            duration_ms: 20,
                            stack: "ysf",
                            carrier_hz: ctx.carrier_hz,
                            slot: 0,
                        };
                        ctx.speech(KEY, spec, &info, ctx.now + k as f64 * 0.02, false, None);
                    }
                }
                if let Some((b, _)) = d {
                    if fich.fn_ < 8 {
                        self.call.blocks.extend_from_slice(&b);
                    }
                    if fich.fn_ == fich.ft {
                        self.text(ctx);
                    }
                }
            }
            2 => {
                if ctx.has_call(KEY) {
                    self.text(ctx);
                    ctx.end_call(KEY, true);
                }
                self.call = Call::default();
            }
            _ => {}
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
        Codec::Ambe2
    }
}

// ---------------------------------------------------------------------------------------------------------------------------
// Generator.

fn frame_bits(f: &Fich, dch: Option<&[u8; 10]>, rng: &mut sdrkit_algo_sim::rng::Rng) -> Vec<u8> {
    let mut b = Vec::with_capacity(960);
    push_u64(&mut b, u64::from_str_radix(FSW, 16).unwrap_or(0), 40);
    b.extend(encode_fich(f));
    let d = encode_dch(dch.unwrap_or(&[0x20; 10]));
    for k in 0..5 {
        b.extend_from_slice(&d[k * 40..k * 40 + 40]);
        let at = b.len();
        b.resize(at + 104, 0);
        rng.fill_bits(&mut b[at..]);
    }
    b
}

fn ten(s: &str) -> [u8; 10] {
    let mut a = [b' '; 10];
    for (i, c) in s.bytes().take(10).enumerate() {
        a[i] = c;
    }
    a
}

/// A transmission: header frames, communication frames with a text or GPS in the data channel, terminator.
pub fn transmission(
    src: &str,
    dest: &str,
    text: &str,
    gps: Option<(f64, f64)>,
    frames: usize,
    rng: &mut sdrkit_algo_sim::rng::Rng,
) -> Vec<u8> {
    let mut b = Vec::new();
    for fn_ in 0..4u8 {
        let f = Fich {
            fi: 0,
            fn_,
            ft: 3,
            dt: 2,
            ..Default::default()
        };
        let d = match fn_ {
            0 => ten(dest),
            1 => ten(src),
            2 => ten("DOWNLINK"),
            _ => ten("UPLINK"),
        };
        b.extend(frame_bits(&f, Some(&d), rng));
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
    let blocks: Vec<[u8; 10]> = data
        .chunks(10)
        .map(|c| {
            let mut a = [0u8; 10];
            a[..c.len()].copy_from_slice(c);
            a
        })
        .collect();
    let ft = (blocks.len() - 1) as u8;
    for k in 0..frames {
        let fn_ = (k % (blocks.len())) as u8;
        let f = Fich {
            fi: 1,
            fn_,
            ft,
            dt: 2,
            ..Default::default()
        };
        b.extend(frame_bits(&f, Some(&blocks[fn_ as usize]), rng));
    }
    let f = Fich {
        fi: 2,
        dt: 2,
        ..Default::default()
    };
    b.extend(frame_bits(&f, None, rng));
    b
}

/// The System Fusion test signal.
pub fn render(seed: u64) -> Vec<Complex32> {
    let mut rng = sdrkit_algo_sim::rng::Rng::new(seed);
    let mut m = FskModulator::new(4800.0, TxPulse::RaisedCosine(0.2), 900.0);
    let mut out = Vec::new();
    silence(0.3, &mut out);
    for (src, dst, text, gps, frames) in [
        ("N0CALL", "ALL", "Fusion test one", None, 20),
        ("N1TEST", "ALL", "", Some((48.85837, 2.29448)), 12),
    ] {
        let bits = transmission(src, dst, text, gps, frames, &mut rng);
        m.symbols(&bits_to_levels(&bits), &mut out);
        m.flush(8, &mut out);
        m.reset();
        silence(0.6, &mut out);
    }
    out
}

/// The System Fusion generator from the settings of the stack.
pub fn generator(cfg: &GenConfig) -> Prepared {
    Prepared::new(render(cfg.seed), cfg.rate_hz, true).with_info(12_500.0, 4800.0, 9600.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_v_d_mode_2_frames_leave_as_info49_speech() {
        assert_eq!(
            speech_of(2),
            Some((Codec::Ambe2, sdrkit_graph::SpeechPackaging::Info49))
        );
        for dt in [0, 1, 3] {
            assert_eq!(speech_of(dt), None, "data type {dt}");
        }
    }

    #[test]
    fn fich_and_dch_round_trip() {
        let f = Fich {
            fi: 1,
            cs: 2,
            cm: 3,
            bn: 0,
            bt: 1,
            fn_: 5,
            ft: 6,
            dt: 2,
            mr: 3,
            sql: true,
            sc: 42,
        };
        let mut s: Vec<f32> = encode_fich(&f)
            .iter()
            .map(|&b| if b != 0 { 1.0 } else { -1.0 })
            .collect();
        s[11] = -s[11];
        assert_eq!(decode_fich(&mut channel(), &s).unwrap().0, f);
        let d = *b"HELLO 1234";
        let s: Vec<f32> = encode_dch(&d)
            .iter()
            .map(|&b| if b != 0 { 1.0 } else { -1.0 })
            .collect();
        assert_eq!(decode_dch(&mut channel(), &s).unwrap().0, d);
    }
}
