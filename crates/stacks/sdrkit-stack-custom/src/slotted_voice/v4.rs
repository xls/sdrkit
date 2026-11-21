//! Frame version 4: 40 ms superframes of 640 symbols at 16000 symbols a second (48 kHz, 3 samples a symbol): a chirp pair,
//! a control channel of 32 QPSK symbols and four slots of 144 symbols with 8 pilots. Each slot word is the (640, 320) code
//! shortened to the profile's information length, rate matched by a circular buffer, scrambled per slot and interleaved.
//! The control word spans 10 superframes.

use super::beacon::{self, Beacon};
use super::phy::{self, K, N};
use crate::family::fit::{parabola, phase_slope};
use crate::family::{crc16_bits, push_bits, push_bytes, read_bits};
use sdrkit_algo_fec::ldpc::LdpcDecoder;
use sdrkit_algo_modem::mmse_dfe::{estimate_channel, MmseDfe};
use sdrkit_codec_custom::voice4;
use sdrkit_core::Complex32;
use sdrkit_util_bits::xorshift::Xorshift64;
use std::f64::consts::TAU;

/// Samples a symbol.
pub const SPS: usize = 3;
/// Symbols a superframe.
pub const SUPER: usize = 640;
/// Samples a superframe.
pub const SUPER_SAMPLES: usize = SPS * SUPER;
/// Superframes in a control cycle.
pub const CYCLE: usize = 10;
/// The first symbol of slot 0.
pub const SLOT0: usize = 64;
/// Symbols a slot.
pub const SLOT: usize = 144;
/// Data symbols a slot.
pub const SLOT_DATA: usize = 136;
/// The pilot seed.
pub const PILOT_SEED: u64 = 0x5049_4C4F_5453_3456;
/// The scrambler seeds of slots 0 to 3 are this plus 1 to 4.
pub const SLOT_SEED: u64 = 0x4B41_414C_3400_0000;
/// The scrambler seed of the control word.
pub const CONTROL_SEED: u64 = 0x4B41_414C_3443_544C;
/// Symbols on each side of a symbol that its re-estimated gain is the mean of.
pub const RE_WINDOW: usize = 12;
/// Symbols on each side of a symbol that its decision-directed phase is smoothed over.
pub const DD_WINDOW: usize = 16;
/// Passes of the phase smoothing: each decides again on the symbols the last one turned.
pub const DD_PASSES: usize = 3;
/// Rounds of re-estimation after a failed first try of a slot.
pub const RE_ROUNDS: usize = 2;
/// Channel taps estimated on the chirps: lags -2 to +2 symbols.
pub const CHANNEL_TAPS: usize = 5;
/// Taps of the multipath equaliser.
pub const EQ_TAPS: usize = 13;
/// The output scale of a symbol.
pub const SCALE: f32 = 0.25 * 1.732_050_8;

/// A slot profile.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Profile {
    /// Fill.
    Idle,
    /// 8.0 kbit/s voice.
    V80,
    /// 6.0 kbit/s voice.
    V60,
    /// 4.8 kbit/s voice.
    V48,
    /// 3.2 kbit/s voice, QPSK.
    V32R,
    /// 34 data bytes.
    D68,
    /// 13 data bytes, QPSK.
    D26R,
}

impl Profile {
    /// Every profile by id.
    pub const ALL: [Profile; 7] = [
        Profile::Idle,
        Profile::V80,
        Profile::V60,
        Profile::V48,
        Profile::V32R,
        Profile::D68,
        Profile::D26R,
    ];

    /// The id.
    pub fn id(self) -> u8 {
        Profile::ALL.iter().position(|&p| p == self).unwrap_or(0) as u8
    }

    /// The profile of an id.
    pub fn of(id: u8) -> Option<Profile> {
        Profile::ALL.get(id as usize).copied()
    }

    /// Information bits.
    pub fn k(self) -> usize {
        match self {
            Profile::Idle | Profile::D68 => 304,
            Profile::V80 => 320,
            Profile::V60 => 240,
            Profile::V48 => 192,
            Profile::V32R => 128,
            Profile::D26R => 136,
        }
    }

    /// Bits a symbol.
    pub fn bits(self) -> usize {
        if matches!(self, Profile::V32R | Profile::D26R) {
            2
        } else {
            4
        }
    }

    /// Coded bits sent.
    pub fn n(self) -> usize {
        SLOT_DATA * self.bits()
    }

    /// True for a voice profile.
    pub fn voice(self) -> bool {
        matches!(self, Profile::V80 | Profile::V60 | Profile::V48 | Profile::V32R)
    }

    /// Data bytes of a data profile.
    pub fn data_bytes(self) -> usize {
        if self == Profile::D26R {
            13
        } else {
            34
        }
    }

    /// The voice codec profile.
    pub fn codec(self) -> voice4::Profile {
        match self {
            Profile::V80 => voice4::Profile::P8_0,
            Profile::V48 => voice4::Profile::P4_8,
            Profile::V32R => voice4::Profile::P3_2,
            _ => voice4::Profile::P6_0,
        }
    }
}

/// Up-chirp symbol `n` (0 to 15).
pub fn chirp(n: usize) -> Complex32 {
    let a = std::f64::consts::PI * ((n as f64 - 8.0).powi(2)) / 16.0;
    Complex32::new(a.cos() as f32, a.sin() as f32)
}

/// The 32 pilots: `[slot][q]`.
pub fn pilots() -> [[Complex32; 8]; 4] {
    let mut g = Xorshift64::new(PILOT_SEED);
    let s = std::f32::consts::FRAC_1_SQRT_2;
    let mut p = [[Complex32::new(0.0, 0.0); 8]; 4];
    for slot in p.iter_mut() {
        for v in slot.iter_mut() {
            let r = g.step();
            *v = Complex32::new(if r & 1 == 1 { -s } else { s }, if r & 2 == 2 { -s } else { s });
        }
    }
    p
}

/// True for the slot positions that hold a pilot.
pub fn is_pilot(i: usize) -> bool {
    i.is_multiple_of(18) && i <= 126
}

/// The data block of a data profile (type, stream, sequence, bytes, CRC-16).
pub fn data_block(profile: Profile, kind: u8, stream: u8, seq: u8, bytes: &[u8]) -> Vec<u8> {
    let mut v = Vec::with_capacity(profile.k());
    push_bits(&mut v, u64::from(kind), 4);
    push_bits(&mut v, u64::from(stream), 4);
    push_bits(&mut v, u64::from(seq), 8);
    let nb = profile.data_bytes();
    for i in 0..nb {
        push_bits(&mut v, u64::from(bytes.get(i).copied().unwrap_or(0)), 8);
    }
    let crc = crc16_bits(&v);
    push_bits(&mut v, u64::from(crc), 16);
    v
}

/// A data block read back: (type, stream, sequence, bytes), when its CRC holds.
pub fn parse_data(profile: Profile, bits: &[u8]) -> Option<(u8, u8, u8, Vec<u8>)> {
    let k = profile.k();
    if bits.len() != k || crc16_bits(&bits[..k - 16]) != read_bits(bits, k - 16, 16) as u16 {
        return None;
    }
    let bytes = (0..profile.data_bytes()).map(|i| read_bits(bits, 16 + 8 * i, 8) as u8).collect();
    Some((read_bits(bits, 0, 4) as u8, read_bits(bits, 4, 4) as u8, read_bits(bits, 8, 8) as u8, bytes))
}

/// The bits sent in slot `slot` for the information bits `info` of `profile` (after scrambling and interleaving).
pub fn slot_bits(slot: usize, profile: Profile, info: &[u8]) -> Vec<u8> {
    let mut cw = [0u8; N];
    phy::encode(&info[..profile.k()], &mut cw);
    sent_bits(slot, profile, &cw)
}

/// The bits sent in slot `slot` for the code word `cw` of `profile`: rate matched, scrambled and interleaved.
pub fn sent_bits(slot: usize, profile: Profile, cw: &[u8; N]) -> Vec<u8> {
    let (k, n) = (profile.k(), profile.n());
    let s = phy::scrambler(SLOT_SEED + slot as u64 + 1, n);
    let t: Vec<u8> = (0..n)
        .map(|j| {
            let pos = if j < k { j } else { K + (j - k) % K };
            cw[pos] ^ s[j]
        })
        .collect();
    let p = phy::permutation(n);
    p.iter().map(|&i| t[i as usize]).collect()
}

/// The 136 symbols of a slot.
pub fn slot_symbols(slot: usize, profile: Profile, info: &[u8]) -> Vec<Complex32> {
    let bits = slot_bits(slot, profile, info);
    let c = if profile.bits() == 2 { phy::qpsk() } else { phy::qam16() };
    let mut out = Vec::with_capacity(SLOT_DATA);
    phy::map(c, &bits, &mut out);
    out
}

/// Decodes the soft slot `llr` (`profile.n()` values) as `profile` in slot `slot`.
pub fn decode_slot(dec: &mut LdpcDecoder, slot: usize, profile: Profile, llr: &[f32], iters: u32) -> Option<Vec<u8>> {
    let k = profile.k();
    let w = decode_slot_word(dec, slot, profile, llr, iters);
    w.good.then(|| w.cw[..k].to_vec())
}

/// A slot word that [`decode_slot_word`] decoded, or failed to decode.
#[derive(Clone, Debug)]
pub struct SlotWord {
    /// The checks and the shortened bits hold.
    pub good: bool,
    /// The hard code word of the last iteration, its shortened bits set to 0.
    pub cw: [u8; N],
    /// The posterior LLRs of the `profile.n()` sent bits (scrambled and interleaved, in the order of `llr`), positive means 0.
    pub sent: Vec<f32>,
}

/// Decodes as [`decode_slot`] does and returns the verdict, the hard code word and the decoder's soft output on the sent bits.
pub fn decode_slot_word(dec: &mut LdpcDecoder, slot: usize, profile: Profile, llr: &[f32], iters: u32) -> SlotWord {
    let (k, n) = (profile.k(), profile.n());
    let p = phy::permutation(n);
    let s = phy::scrambler(SLOT_SEED + slot as u64 + 1, n);
    let at = |j: usize| if j < k { j } else { K + (j - k) % K };
    let mut t = vec![0f32; n];
    for (i, &pi) in p.iter().enumerate() {
        t[pi as usize] = llr[i];
    }
    let mut acc = [0f32; N];
    for j in 0..n {
        acc[at(j)] += if s[j] == 1 { -t[j] } else { t[j] };
    }
    let mean = acc.iter().map(|v| v.abs()).sum::<f32>() / n as f32;
    let scale = 12.0 / mean.max(1e-12);
    let mut l = [0i8; N];
    for (o, &v) in l.iter_mut().zip(&acc) {
        *o = phy::quantise(v, scale);
    }
    let (good, mut cw) = phy::decode_word(dec, &l, k, iters);
    cw[k..K].fill(0);
    let mut post = [0i16; N];
    if dec.posterior(&mut post).is_err() {
        // A decoder without soft output: the hard word at the strength of the input.
        for (o, &b) in post.iter_mut().zip(&cw) {
            *o = if b == 0 { 12 } else { -12 };
        }
    }
    let sent = p
        .iter()
        .map(|&pi| {
            let j = pi as usize;
            let v = f32::from(post[at(j)]) / scale;
            if s[j] == 1 { -v } else { v }
        })
        .collect();
    SlotWord { good, cw, sent }
}

/// The symbols of a slot (144, pilots and data) with the gain and phase of each re-estimated from the soft output `sent` of
/// a failed decode of `profile` (the posterior LLRs of the sent bits, as [`SlotWord::sent`]): every symbol is divided by
/// `sum z conj(s) / sum E|s|^2` over the symbols up to [`RE_WINDOW`] away, where `s` is the pilot or the mean point of the
/// symbol's bits and `E|s|^2` its mean energy, so a symbol the decoder is unsure of weighs little.
pub fn re_estimate(z: &[Complex32], pilots: &[Complex32; 8], profile: Profile, sent: &[f32]) -> Vec<Complex32> {
    let c = if profile.bits() == 2 { phy::qpsk() } else { phy::qam16() };
    let mut di = sent.chunks_exact(c.bits()).map(|b| c.soft_point(b).unwrap_or_default());
    let soft: Vec<(Complex32, f32)> = (0..SLOT).map(|i| if is_pilot(i) { (pilots[i / 18], pilots[i / 18].norm_sqr()) } else { di.next().unwrap_or_default() }).collect();
    // Running sums of the products and of the sent energy.
    let mut acc = vec![(Complex32::new(0.0, 0.0), 0f32); SLOT + 1];
    for i in 0..SLOT {
        acc[i + 1] = (acc[i].0 + z[i] * soft[i].0.conj(), acc[i].1 + soft[i].1);
    }
    (0..SLOT)
        .map(|i| {
            let (lo, hi) = (i.saturating_sub(RE_WINDOW), (i + RE_WINDOW + 1).min(SLOT));
            let g = (acc[hi].0 - acc[lo].0) / (acc[hi].1 - acc[lo].1).max(1e-12);
            z[i] / if g.norm() > 1e-6 { g } else { Complex32::new(1.0, 0.0) }
        })
        .collect()
}

/// Turns the symbols `z[from..]` by a phase smoothed over decisions: the phase of symbol `i` is that of the sum of
/// `z[j] conj(d[j])` over the symbols up to [`DD_WINDOW`] away, where `d[j]` is `known(j)` when it gives a symbol (a pilot)
/// and else the nearest point of `decide(j)`. Points on the diagonals stand for QPSK too, so one 16-QAM decision serves
/// slots of either order.
pub fn smooth_phase(
    z: &mut [Complex32],
    from: usize,
    known: impl Fn(usize) -> Option<Complex32>,
    decide: impl Fn(usize) -> &'static sdrkit_algo_modem::Constellation,
) {
    let n = z.len() - from;
    let mut acc = vec![Complex32::new(0.0, 0.0); n + 1];
    for i in 0..n {
        let v = z[from + i];
        let d = known(from + i).unwrap_or_else(|| decide(from + i).nearest(v));
        acc[i + 1] = acc[i] + v * d.conj();
    }
    for i in 0..n {
        let (lo, hi) = (i.saturating_sub(DD_WINDOW), (i + DD_WINDOW + 1).min(n));
        let e = acc[hi] - acc[lo];
        if e.norm() > 1e-12 {
            z[from + i] *= e.conj() / e.norm();
        }
    }
}

/// The fields of a control word.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Control {
    /// The profile id of each slot.
    pub map: [u8; 4],
    /// Message type: 0 beacon, 1 slot callsign, 2 text, 15 empty.
    pub kind: u8,
    /// Part index of a text.
    pub part: u8,
    /// Number of parts of a text.
    pub parts: u8,
    /// The 33 message bytes.
    pub bytes: [u8; 33],
}

/// The 320 information bits of a control word.
pub fn control_bits(c: &Control) -> Vec<u8> {
    let mut v = Vec::with_capacity(K);
    push_bits(&mut v, 4, 8);
    for &m in &c.map {
        push_bits(&mut v, u64::from(m), 4);
    }
    push_bits(&mut v, u64::from(c.kind), 4);
    push_bits(&mut v, u64::from(c.part), 4);
    push_bits(&mut v, u64::from(c.parts.max(1) - 1), 4);
    push_bits(&mut v, 0, 4);
    push_bytes(&mut v, &c.bytes);
    let crc = crc16_bits(&v);
    push_bits(&mut v, u64::from(crc), 16);
    v
}

/// The control word of 320 bits, when its CRC holds.
pub fn parse_control(bits: &[u8]) -> Option<Control> {
    if bits.len() != K || crc16_bits(&bits[..304]) != read_bits(bits, 304, 16) as u16 {
        return None;
    }
    Some(Control {
        map: std::array::from_fn(|i| read_bits(bits, 8 + 4 * i, 4) as u8),
        kind: read_bits(bits, 24, 4) as u8,
        part: read_bits(bits, 28, 4) as u8,
        parts: read_bits(bits, 32, 4) as u8 + 1,
        bytes: std::array::from_fn(|i| read_bits(bits, 40 + 8 * i, 8) as u8),
    })
}

/// The 640 sent bits of a control word.
pub fn control_sent(c: &Control) -> Vec<u8> {
    let mut cw = [0u8; N];
    phy::encode(&control_bits(c), &mut cw);
    let ks = phy::scrambler(CONTROL_SEED, N);
    let p = phy::permutation(N);
    p.iter().map(|&i| cw[i as usize] ^ ks[i as usize]).collect()
}

/// The transmit options of version 4.
#[derive(Clone, Debug, PartialEq)]
pub struct TxOptions {
    /// The voice profiles of the voice slots, from slot 0 on: 0 to 4 voice slots, each with its own profile. Every voice slot
    /// carries the same speech.
    pub voice: Vec<Profile>,
    /// Robust data slots.
    pub robust: bool,
    /// The callsign of the slots.
    pub callsign: String,
    /// The beacon sent in the control channel.
    pub beacon: Option<Beacon>,
    /// A text queued every 2 s.
    pub text: String,
}

/// The version 4 transmitter: 16 kHz speech in, 48 kHz samples out.
pub struct Transmitter {
    o: TxOptions,
    /// One encoder for each voice profile in use.
    enc: Vec<(Profile, voice4::Encoder)>,
    pcm: Vec<i16>,
    h: Vec<f32>,
    tail: Vec<Complex32>,
    sf: u64,
    pilots: [[Complex32; 8]; 4],
    control: Vec<u8>,
    words: u64,
    ctrl_text: Option<(Vec<u8>, usize)>,
    slot_text: Option<(Vec<u8>, usize)>,
    seq: u8,
    turn: usize,
    prev_all_voice: bool,
}

impl Transmitter {
    /// A transmitter with these options.
    pub fn new(o: &TxOptions) -> Transmitter {
        let mut o = o.clone();
        o.voice.retain(|p| p.voice());
        o.voice.truncate(4);
        let mut enc: Vec<(Profile, voice4::Encoder)> = Vec::new();
        for &p in &o.voice {
            if enc.iter().any(|e| e.0 == p) {
                continue;
            }
            let config = voice4::Config {
                rate: voice4::SampleRate::Hz16000,
                frame: voice4::FrameLength::Ms40,
                profile: p.codec(),
                type_in_band: false,
            };
            if let Ok(e) = voice4::Encoder::new(config, voice4::EncoderSettings::default()) {
                enc.push((p, e));
            }
        }
        Transmitter {
            o,
            enc,
            pcm: Vec::new(),
            h: phy::rrc(SPS, 10, 0.25),
            tail: vec![Complex32::new(0.0, 0.0); 31],
            sf: 0,
            pilots: pilots(),
            control: Vec::new(),
            words: 0,
            ctrl_text: None,
            slot_text: None,
            seq: 0,
            turn: 0,
            prev_all_voice: false,
        }
    }

    /// Takes 16 kHz speech and appends the samples of every superframe it completes.
    pub fn push(&mut self, pcm: &[i16], out: &mut Vec<Complex32>) {
        self.pcm.extend_from_slice(pcm);
        while self.pcm.len() >= 640 {
            let f: Vec<i16> = self.pcm.drain(..640).collect();
            self.superframe(Some(&f), out);
        }
    }

    /// Ends the stream: the rest of the speech in one last superframe, then the filter tail.
    pub fn finish(&mut self, out: &mut Vec<Complex32>) {
        if !self.pcm.is_empty() {
            let mut f = std::mem::take(&mut self.pcm);
            f.resize(640, 0);
            self.superframe(Some(&f), out);
        }
        out.extend(self.tail.iter().take(31 - SPS));
        self.tail.iter_mut().for_each(|v| *v = Complex32::new(0.0, 0.0));
    }

    fn next_control(&mut self, map: [u8; 4]) -> Control {
        let c = self.words;
        self.words += 1;
        let mut bytes = [0u8; 33];
        let mut kind = 15;
        let (mut part, mut parts) = (0, 1);
        if self.ctrl_text.is_none() && self.prev_all_voice {
            if let Some((t, _)) = self.slot_text.take() {
                let t: Vec<u8> = t.into_iter().skip(2).take(528).collect();
                if !t.is_empty() {
                    self.ctrl_text = Some((t, 0));
                }
            }
        }
        let beacon_due = self.o.beacon.is_some() && c.is_multiple_of(4);
        if let Some((t, p)) = self.ctrl_text.as_mut().filter(|_| !beacon_due) {
            let n = t.len().div_ceil(33).min(16);
            let chunk = &t[33 * *p..(33 * (*p + 1)).min(t.len())];
            bytes[..chunk.len()].copy_from_slice(chunk);
            kind = 2;
            part = *p as u8;
            parts = n as u8;
            *p += 1;
            if *p >= n {
                self.ctrl_text = None;
            }
        } else if !self.o.voice.is_empty() && (c % 4 == 2 || self.o.beacon.is_none()) {
            kind = 1;
            let slot = self.turn % self.o.voice.len();
            self.turn += 1;
            bytes[0] = slot as u8;
            for (i, b) in self.o.callsign.bytes().take(32).enumerate() {
                bytes[1 + i] = b;
            }
        } else if let Some(b) = &self.o.beacon {
            kind = 0;
            if let Ok(bits) = beacon::frame_bits(b, true) {
                let mut padded = bits;
                padded.resize(33 * 8, 0);
                for (i, v) in bytes.iter_mut().enumerate() {
                    *v = read_bits(&padded, 8 * i, 8) as u8;
                }
            }
        }
        Control {
            map,
            kind,
            part,
            parts,
            bytes,
        }
    }

    fn superframe(&mut self, pcm: Option<&[i16]>, out: &mut Vec<Complex32>) {
        let q = (self.sf % CYCLE as u64) as usize;
        if self.sf.is_multiple_of(50) && self.slot_text.is_none() && !self.o.text.is_empty() {
            let t = self.o.text.as_bytes();
            let mut m = (t.len() as u16).to_be_bytes().to_vec();
            m.extend_from_slice(t);
            self.slot_text = Some((m, 0));
        }
        // One voice payload for each voice profile, from the same speech.
        let mut payloads: Vec<(Profile, Vec<u8>)> = Vec::new();
        if let Some(pcm) = pcm {
            for (p, enc) in &mut self.enc {
                let mut buf = vec![0u8; 64];
                if let Ok(voice4::Encoded::Speech(n)) = enc.encode(pcm, &mut buf) {
                    let mut bits = Vec::new();
                    push_bytes(&mut bits, &buf[..n]);
                    bits.resize(p.k(), 0);
                    payloads.push((*p, bits));
                }
            }
        }
        let data_profile = if self.o.robust { Profile::D26R } else { Profile::D68 };
        let mut map = [0u8; 4];
        let mut infos: Vec<(Profile, Vec<u8>)> = Vec::with_capacity(4);
        for s in 0..4 {
            if let Some((p, bits)) = self.o.voice.get(s).and_then(|v| payloads.iter().find(|q| q.0 == *v)) {
                map[s] = p.id();
                infos.push((*p, bits.clone()));
                continue;
            }
            map[s] = data_profile.id();
            let nb = data_profile.data_bytes();
            let mut info = None;
            if let Some((t, at)) = self.slot_text.as_mut() {
                let chunk: Vec<u8> = t[*at..(*at + nb).min(t.len())].to_vec();
                let kind = if *at == 0 { 1 } else { 4 };
                *at += chunk.len();
                info = Some(data_block(data_profile, kind, 0, self.seq, &chunk));
                self.seq = self.seq.wrapping_add(1);
                if *at >= t.len() {
                    self.slot_text = None;
                }
            }
            let p = if info.is_some() || self.o.robust { data_profile } else { Profile::Idle };
            infos.push((p, info.unwrap_or_else(|| data_block(p, 0, 0, 0, &[]))));
        }
        self.prev_all_voice = infos.iter().all(|(p, _)| p.voice());
        if q == 0 {
            let c = self.next_control(map);
            self.control = control_sent(&c);
        }
        let mut sym = Vec::with_capacity(SUPER);
        sym.extend((0..16).map(chirp));
        let sign = if q == 0 { -1.0 } else { 1.0 };
        sym.extend((0..16).map(|n| chirp(n).conj() * sign));
        let ctrl = &self.control[64 * q..64 * q + 64];
        phy::map(phy::qpsk(), ctrl, &mut sym);
        for (s, (p, info)) in infos.iter().enumerate() {
            let d = slot_symbols(s, *p, info);
            let mut di = d.into_iter();
            for i in 0..SLOT {
                sym.push(if is_pilot(i) { self.pilots[s][i / 18] } else { di.next().unwrap_or_default() });
            }
        }
        // Overlap-add: symbol m starts its 31 taps at sample 3 m.
        let mut buf = vec![Complex32::new(0.0, 0.0); SUPER_SAMPLES + 31];
        buf[..31].copy_from_slice(&self.tail);
        for (m, &s) in sym.iter().enumerate() {
            let s = s * SCALE;
            for (i, &t) in self.h.iter().enumerate() {
                buf[SPS * m + i] += s * t;
            }
        }
        out.extend_from_slice(&buf[..SUPER_SAMPLES]);
        self.tail.copy_from_slice(&buf[SUPER_SAMPLES..SUPER_SAMPLES + 31]);
        self.sf += 1;
    }
}

/// What the version 4 receiver reports.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Report {
    /// Locked to the superframes.
    pub locked: bool,
    /// The carrier offset, Hz.
    pub carrier_hz: f64,
    /// SNR of the known symbols, dB.
    pub snr_db: Option<f64>,
    /// Slots decoded.
    pub slots_good: u64,
    /// Slots that failed.
    pub slots_bad: u64,
    /// The profile and callsign of each slot.
    pub slots: [(Option<Profile>, Option<String>); 4],
    /// Control words decoded.
    pub controls: u64,
}

/// One output of the version 4 receiver.
#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    /// 640 samples of 16 kHz speech of the voice slot played.
    Speech(Vec<i16>),
    /// A beacon from the control channel.
    Beacon(Beacon),
    /// A text, from the control channel or the data slots.
    Text(Vec<u8>),
}

/// The passes of the receiver that can be left out, to measure what each gives.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RxOptions {
    /// The multipath equaliser.
    pub equalise: bool,
    /// The re-estimation of a slot from a failed decode.
    pub re_estimate: bool,
    /// The decision-directed phase smoothing over the data symbols.
    pub smooth_phase: bool,
}

impl Default for RxOptions {
    fn default() -> Self {
        RxOptions {
            equalise: true,
            re_estimate: true,
            smooth_phase: true,
        }
    }
}

/// The channel on the chirps, averaged over the superframes since the lock.
#[derive(Clone, Copy, Debug)]
struct Channel {
    /// The taps at lags -2 to +2 symbols, of the symbols after the gain and phase fit.
    taps: [Complex32; CHANNEL_TAPS],
    /// The noise variance of the fit.
    noise: f32,
    /// Superframes averaged.
    count: u32,
}

/// Samples of the chirps that the channel fit uses: those whose five symbols are all chirp symbols.
const CHANNEL_ROWS: f32 = 40.0;

/// Superframes searched for the lock.
const ACQ_SUPERS: usize = 3;

/// The version 4 receiver at 48 kHz.
pub struct Receiver {
    x: Vec<Complex32>,
    base: u64,
    phase: f64,
    f_hz: f64,
    h: Vec<f32>,
    pilots: [[Complex32; 8]; 4],
    /// The start of the next superframe (absolute, fractional), once locked.
    next: Option<f64>,
    q: Option<usize>,
    ctrl: Vec<Option<Vec<f32>>>,
    weak: u32,
    dec: LdpcDecoder,
    last: [Option<Profile>; 4],
    hint: [Option<Profile>; 4],
    codec: Option<(Profile, voice4::Decoder)>,
    play: Option<usize>,
    text: Option<(Vec<u8>, usize, u8)>,
    ctrl_text: Option<(Vec<u8>, u8)>,
    nv: f64,
    supers: u64,
    /// Superframes since a slot or a control word last decoded.
    since_good: u32,
    /// Good slots since the lock.
    confirmed: u32,
    opts: RxOptions,
    chan: Option<Channel>,
    /// The latest report.
    pub report: Report,
}

impl Default for Receiver {
    fn default() -> Self {
        Self::new()
    }
}

impl Receiver {
    /// A receiver that searches.
    pub fn new() -> Receiver {
        Self::with_options(RxOptions::default())
    }

    /// A receiver that searches, with the passes of `opts`.
    pub fn with_options(opts: RxOptions) -> Receiver {
        Receiver {
            opts,
            chan: None,
            x: Vec::new(),
            base: 0,
            phase: 0.0,
            f_hz: 0.0,
            h: phy::rrc(SPS, 10, 0.25),
            pilots: pilots(),
            next: None,
            q: None,
            ctrl: vec![None; CYCLE],
            weak: 0,
            dec: phy::decoder(),
            last: [None; 4],
            hint: [None; 4],
            codec: None,
            play: None,
            text: None,
            ctrl_text: None,
            nv: 0.0,
            supers: 0,
            since_good: 0,
            confirmed: 0,
            report: Report::default(),
        }
    }

    /// Forgets the counters.
    pub fn clear(&mut self) {
        let (locked, carrier) = (self.report.locked, self.report.carrier_hz);
        self.report = Report {
            locked,
            carrier_hz: carrier,
            ..Report::default()
        };
    }

    fn unlock(&mut self) {
        self.next = None;
        self.q = None;
        self.chan = None;
        self.report.locked = false;
        self.f_hz = 0.0;
    }

    /// Takes 48 kHz samples and appends the outputs.
    pub fn process(&mut self, input: &[Complex32], out: &mut Vec<Event>) {
        for &v in input {
            let a = -TAU * self.phase;
            self.x.push(v * Complex32::new(a.cos() as f32, a.sin() as f32));
            self.phase = (self.phase + self.f_hz / 48_000.0).rem_euclid(1.0);
        }
        if self.next.is_none() {
            let need = (ACQ_SUPERS + 1) * SUPER_SAMPLES + 200;
            if self.x.len() >= need {
                self.acquire();
                if self.next.is_none() {
                    let drop = self.x.len() - SUPER_SAMPLES * ACQ_SUPERS;
                    self.x.drain(..drop);
                    self.base += drop as u64;
                }
            }
        }
        while let Some(t) = self.next {
            let end = (self.base + self.x.len() as u64) as f64;
            if t + (SUPER_SAMPLES + 16 * SPS + 40) as f64 > end {
                break;
            }
            self.superframe(t, out);
        }
        if let Some(t) = self.next {
            let keep = (t as i64 - 64 - self.base as i64).max(0) as usize;
            if keep > 0 {
                self.x.drain(..keep.min(self.x.len()));
                self.base += keep as u64;
            }
        }
    }

    /// Moves the mixer by `df` Hz: the buffered samples are mixed again, so the next superframe sees the change.
    fn retune(&mut self, df: f64) {
        for (i, v) in self.x.iter_mut().enumerate() {
            let a = -TAU * (df * i as f64 / 48_000.0).fract();
            *v *= Complex32::new(a.cos() as f32, a.sin() as f32);
        }
        self.phase = (self.phase + df * self.x.len() as f64 / 48_000.0).rem_euclid(1.0);
        self.f_hz += df;
        self.report.carrier_hz = self.f_hz;
    }

    fn mf(&self, t: i64) -> Complex32 {
        let s = t - 15 - self.base as i64;
        if s < 0 || s as usize + self.h.len() > self.x.len() {
            return Complex32::new(0.0, 0.0);
        }
        let s = s as usize;
        self.x[s..s + self.h.len()].iter().zip(&self.h).map(|(a, &b)| a * b).sum()
    }

    fn mf_at(&self, pos: f64) -> Complex32 {
        let fl = pos.floor() as i64;
        let w = [self.mf(fl - 1), self.mf(fl), self.mf(fl + 1), self.mf(fl + 2)];
        sdrkit_algo_dsp::interp::Reader::new(sdrkit_algo_dsp::interp::Cubic).at(&w, 1.0 + (pos - fl as f64))
    }

    /// The normalised correlation with the up (or down) chirp of 16 symbols starting at `t`.
    fn chirp_corr(&self, z: &[Complex32], t: usize, down: bool) -> Complex32 {
        let (mut c, mut e) = (Complex32::new(0.0, 0.0), 0f32);
        for n in 0..16 {
            let v = z[t + SPS * n];
            let r = if down { chirp(n) } else { chirp(n).conj() };
            c += v * r;
            e += v.norm_sqr();
        }
        c / (16.0 * e).sqrt().max(1e-20)
    }

    fn acquire(&mut self) {
        let n = self.x.len();
        let z: Vec<Complex32> = (0..n as i64).map(|t| self.mf(t + self.base as i64)).collect();
        let span = 16 * SPS;
        let cu: Vec<f32> = (0..n - span).map(|t| self.chirp_corr(&z, t, false).norm()).collect();
        let cd: Vec<f32> = (0..n - span).map(|t| self.chirp_corr(&z, t, true).norm()).collect();
        let mut best = (0f32, 0usize, 0i64);
        for t0 in 20..20 + SUPER_SAMPLES {
            for j in -13i64..=13 {
                let mut s = 0f32;
                for k in 0..ACQ_SUPERS {
                    let u = (t0 as i64 - j) as usize + k * SUPER_SAMPLES;
                    let d = (t0 as i64 + 48 + j) as usize + k * SUPER_SAMPLES;
                    if d >= cd.len() || u >= cu.len() {
                        s = 0.0;
                        break;
                    }
                    s += cu[u] + cd[d];
                }
                let m = s / (2 * ACQ_SUPERS) as f32;
                if m > best.0 {
                    best = (m, t0, j);
                }
            }
        }
        if best.0 < 0.30 {
            return;
        }
        let (t0, j) = (best.1 as i64, best.2);
        let refine = |c: &[f32], p: i64| -> f64 {
            let p = p as usize;
            if p == 0 || p + 1 >= c.len() {
                return p as f64;
            }
            p as f64 + parabola(f64::from(c[p - 1]), f64::from(c[p]), f64::from(c[p + 1]))
        };
        let up = refine(&cu, t0 - j);
        let dn = refine(&cd, t0 + 48 + j);
        let f = (dn - up - 48.0) / 6.0 * 1000.0;
        let start = (up + dn) / 2.0 - 24.0 + self.base as f64;
        self.retune(f);
        self.next = Some(start);
        self.chan = None;
        self.since_good = 0;
        self.confirmed = 0;
        self.supers = 0;
        self.weak = 0;
    }

    fn superframe(&mut self, t: f64, out: &mut Vec<Event>) {
        // Timing from the chirps at -1, 0, +1 sample.
        let metric = |s: &Receiver, dt: f64| {
            let (mut cu, mut cd) = (Complex32::new(0.0, 0.0), Complex32::new(0.0, 0.0));
            for n in 0..16 {
                cu += s.mf_at(t + dt + (SPS * n) as f64) * chirp(n).conj();
                cd += s.mf_at(t + dt + (SPS * (16 + n)) as f64) * chirp(n);
            }
            (cu.norm() + cd.norm(), cu, cd)
        };
        let (m0, cu, cd) = metric(self, 0.0);
        let (mm, _, _) = metric(self, -1.0);
        let (mp, _, _) = metric(self, 1.0);
        let dt = parabola(f64::from(mm), f64::from(m0), f64::from(mp)).clamp(-1.5, 1.5);
        let t = t + dt;
        let y: Vec<Complex32> = (0..SUPER + 16).map(|i| self.mf_at(t + (SPS * i) as f64)).collect();
        // The marker: the down-chirp sign against the up-chirp.
        let marker = (cd * cu.conj()).re < 0.0;
        if marker {
            self.q = Some(0);
        }
        let mut known: Vec<(f64, Complex32, Complex32)> = Vec::with_capacity(16 * 3 + 32);
        for n in 0..16 {
            known.push((n as f64, y[n], chirp(n)));
            let d = chirp(n).conj() * if marker { -1.0 } else { 1.0 };
            known.push(((16 + n) as f64, y[16 + n], d));
            known.push(((SUPER + n) as f64, y[SUPER + n], chirp(n)));
        }
        for s in 0..4 {
            for q in 0..8 {
                let i = SLOT0 + SLOT * s + 18 * q;
                known.push((i as f64, y[i], self.pilots[s][q]));
            }
        }
        // A phase that turns linearly and a gain, fitted to every known symbol.
        let tc = 320.0;
        let v: Vec<(f64, Complex32)> = known.iter().map(|&(p, yy, k)| (p - tc, yy * k.conj())).collect();
        let (w1, _) = phase_slope(&v, 0.0, 0.08, 0.0005);
        let (w, sum) = phase_slope(&v, w1, 0.0006, 0.00002);
        let a = sum / v.len() as f32;
        let rot = |p: f64| Complex32::new((w * p).cos() as f32, (w * p).sin() as f32);
        let resid: f64 = known
            .iter()
            .map(|&(p, yy, k)| f64::from((yy - a * rot(p - tc) * k).norm_sqr()))
            .sum::<f64>()
            / (known.len() - 2) as f64
            / f64::from(a.norm_sqr()).max(1e-20);
        self.nv = if self.supers == 0 { resid } else { 0.8 * self.nv + 0.2 * resid };
        self.supers += 1;
        self.report.snr_db = Some(-10.0 * self.nv.max(1e-6).log10());
        let eu: f32 = y[..16].iter().map(|v| v.norm_sqr()).sum();
        let ed: f32 = y[16..32].iter().map(|v| v.norm_sqr()).sum();
        let _ = m0;
        let strength = 0.5 * (cu.norm() / (16.0 * eu).sqrt().max(1e-20) + cd.norm() / (16.0 * ed).sqrt().max(1e-20));
        self.weak = if strength < 0.12 { self.weak + 1 } else { 0 };
        // A share of the phase slope goes to the mixer; the buffered samples follow at once.
        let df = w / TAU * 16_000.0;
        let gain = if self.supers <= 3 { 1.0 } else { 0.35 };
        self.retune(gain * df.clamp(-300.0, 300.0));
        self.next = Some(t + SUPER_SAMPLES as f64);
        // The symbols after the gain and phase fit.
        let mut z: Vec<Complex32> = (0..SUPER + 16)
            .map(|i| {
                let g = a * rot(i as f64 - tc);
                y[i] / if g.norm() > 1e-12 { g } else { Complex32::new(1.0, 0.0) }
            })
            .collect();
        if self.opts.equalise {
            let sent: Vec<Option<Complex32>> = (0..SUPER + 16)
                .map(|i| match i {
                    0..16 => Some(chirp(i)),
                    16..32 => Some(chirp(i - 16).conj() * if marker { -1.0 } else { 1.0 }),
                    SUPER.. => Some(chirp(i - SUPER)),
                    _ => None,
                })
                .collect();
            self.multipath(&mut z, &sent);
        }
        if self.opts.smooth_phase {
            let pilots = self.pilots;
            let known = |i: usize| {
                let j = i.checked_sub(SLOT0)?;
                (j < 4 * SLOT && is_pilot(j % SLOT)).then(|| pilots[j / SLOT][(j % SLOT) / 18])
            };
            for _ in 0..DD_PASSES {
                smooth_phase(&mut z[..SUPER], 32, known, |i| if i < SLOT0 { phy::qpsk() } else { phy::qam16() });
            }
        }
        let eq = |i: usize| z[i];
        let nv = self.nv.max(1e-4) as f32;
        // Control symbols.
        if let Some(q) = self.q {
            let ctrl: Vec<Complex32> = (32..64).map(eq).collect();
            let mut l = Vec::new();
            phy::demap(phy::qpsk(), &ctrl, nv, &mut l);
            self.ctrl[q] = Some(l);
            if q + 1 == CYCLE {
                self.control(out);
            }
            self.q = Some((q + 1) % CYCLE);
        }
        // Slots.
        let mut payloads: Vec<(usize, Profile, Vec<u8>)> = Vec::new();
        for s in 0..4 {
            let d: Vec<Complex32> = (0..SLOT).filter(|&i| !is_pilot(i)).map(|i| eq(SLOT0 + SLOT * s + i)).collect();
            let mut tries: Vec<(Profile, u32)> = Vec::new();
            for p in [self.hint[s], self.last[s]].into_iter().flatten() {
                if !tries.iter().any(|t| t.0 == p) {
                    tries.push((p, if tries.is_empty() { 40 } else { 20 }));
                }
            }
            for p in [Profile::D68, Profile::V80, Profile::V60, Profile::V48, Profile::V32R, Profile::D26R] {
                if !tries.iter().any(|t| t.0 == p) {
                    tries.push((p, if tries.is_empty() { 40 } else { 20 }));
                }
            }
            let mut got = None;
            let mut l16 = Vec::new();
            let mut l4 = Vec::new();
            phy::demap(phy::qam16(), &d, nv, &mut l16);
            phy::demap(phy::qpsk(), &d, nv, &mut l4);
            for (first, (p, iters)) in tries.into_iter().enumerate().map(|(i, t)| (i == 0, t)) {
                let l = if p.bits() == 2 { &l4 } else { &l16 };
                let mut w = decode_slot_word(&mut self.dec, s, p, l, iters);
                if !w.good && first && self.opts.re_estimate {
                    // The soft output of the failed word is close to the word sent: it gives the gain and phase of each
                    // symbol, and the slot is read again.
                    let zs = &z[SLOT0 + SLOT * s..SLOT0 + SLOT * (s + 1)];
                    let c = if p.bits() == 2 { phy::qpsk() } else { phy::qam16() };
                    for _ in 0..RE_ROUNDS {
                        let again: Vec<Complex32> = re_estimate(zs, &self.pilots[s], p, &w.sent)
                            .into_iter()
                            .enumerate()
                            .filter(|(i, _)| !is_pilot(*i))
                            .map(|(_, v)| v)
                            .collect();
                        let mut l = Vec::new();
                        phy::demap(c, &again, nv, &mut l);
                        w = decode_slot_word(&mut self.dec, s, p, &l, iters);
                        if w.good {
                            break;
                        }
                    }
                }
                if w.good {
                    let bits = w.cw[..p.k()].to_vec();
                    if !p.voice() {
                        let Some((kind, stream, seq, bytes)) = parse_data(p, &bits) else {
                            continue;
                        };
                        let p = if kind == 0 && p == Profile::D68 { Profile::Idle } else { p };
                        got = Some(p);
                        if stream == 0 {
                            self.data(kind, seq, &bytes, out);
                        }
                    } else {
                        // A constant input decodes to a word of one value; speech never codes it.
                        if bits.iter().all(|&b| b == bits[0]) {
                            continue;
                        }
                        got = Some(p);
                        payloads.push((s, p, bits));
                    }
                    break;
                }
            }
            match got {
                Some(p) => {
                    self.report.slots_good += 1;
                    self.since_good = 0;
                    self.confirmed += 1;
                    // Four good slots confirm the lock.
                    self.report.locked |= self.confirmed >= 4;
                    self.last[s] = Some(p);
                    self.report.slots[s].0 = Some(p);
                }
                None => self.report.slots_bad += u64::from(self.report.locked),
            }
        }
        // Play the first slot that carries voice, and keep it while it does.
        if self.play.is_none_or(|s| !payloads.iter().any(|p| p.0 == s)) {
            self.play = payloads.first().map(|p| p.0);
        }
        if let Some((_, p, bits)) = payloads.iter().find(|p| Some(p.0) == self.play) {
            if self.codec.as_ref().is_none_or(|c| c.0 != *p) {
                self.codec = voice4::Decoder::new(
                    voice4::Config {
                        rate: voice4::SampleRate::Hz16000,
                        frame: voice4::FrameLength::Ms40,
                        profile: p.codec(),
                        type_in_band: false,
                    },
                    voice4::DecoderSettings::default(),
                )
                .ok()
                .map(|d| (*p, d));
            }
            if let Some((_, d)) = self.codec.as_mut() {
                let bytes = crate::family::pack(bits);
                let mut pcm = vec![0i16; d.frame_samples()];
                let _ = d.decode(voice4::Input::Frame(&bytes), voice4::Damage::default(), &mut pcm);
                out.push(Event::Speech(pcm));
            }
        }
        self.since_good += 1;
        // A lock that decodes nothing for a second is not a version 4 signal.
        if self.weak >= 8 || self.since_good > 25 {
            self.unlock();
        }
    }

    /// Fits the channel taps on the chirps of the symbols `z` (`sent` holds the chirp symbols), averages them over the
    /// superframes, and equalises `z` when the taps beside the centre hold more than 1/50 of its energy (and more than three
    /// times what the noise of the fit puts there).
    fn multipath(&mut self, z: &mut Vec<Complex32>, sent: &[Option<Complex32>]) {
        let Ok((h, noise)) = estimate_channel(z, sent, -2, CHANNEL_TAPS) else {
            return;
        };
        let c = self.chan.get_or_insert(Channel {
            taps: [Complex32::new(0.0, 0.0); CHANNEL_TAPS],
            noise: 0.0,
            count: 0,
        });
        c.count += 1;
        // A plain mean of the first superframes, then a smoothing of 0.2.
        let w = (1.0 / c.count as f32).max(0.2);
        for (t, v) in c.taps.iter_mut().zip(&h) {
            *t += (*v - *t) * w;
        }
        c.noise += (noise - c.noise) * w;
        let centre = c.taps[CHANNEL_TAPS / 2].norm_sqr();
        let side = c.taps.iter().map(|t| t.norm_sqr()).sum::<f32>() - centre;
        let floor = 3.0 * (CHANNEL_TAPS - 1) as f32 * c.noise / CHANNEL_ROWS * (w / (2.0 - w));
        if side <= (centre / 50.0).max(floor) {
            return;
        }
        let Ok(d) = MmseDfe::linear(&c.taps, -2, -(EQ_TAPS as i32 / 2), EQ_TAPS, c.noise.max(1e-3)) else {
            return;
        };
        let mut out = Vec::with_capacity(z.len());
        d.run(z, &[], |v| v, &mut out);
        *z = out;
    }

    fn data(&mut self, kind: u8, seq: u8, bytes: &[u8], out: &mut Vec<Event>) {
        match kind {
            1 => {
                let len = usize::from(u16::from_be_bytes([bytes[0], bytes[1]])) + 2;
                self.text = Some((bytes.to_vec(), len, seq));
            }
            4 => match self.text.as_mut() {
                Some((t, _, last)) if seq == last.wrapping_add(1) => {
                    t.extend_from_slice(bytes);
                    *last = seq;
                }
                _ => self.text = None,
            },
            _ => {}
        }
        if let Some((t, len, _)) = &self.text {
            if t.len() >= *len {
                out.push(Event::Text(t[2..*len].to_vec()));
                self.text = None;
            }
        }
    }

    fn control(&mut self, out: &mut Vec<Event>) {
        if self.ctrl.iter().any(Option::is_none) {
            self.ctrl.iter_mut().for_each(|c| *c = None);
            return;
        }
        let sent: Vec<f32> = self.ctrl.iter_mut().flat_map(|c| c.take().unwrap_or_default()).collect();
        let ks = phy::scrambler(CONTROL_SEED, N);
        let p = phy::permutation(N);
        let mut cw = [0f32; N];
        for (i, &pi) in p.iter().enumerate() {
            let v = sent[i];
            cw[pi as usize] = if ks[pi as usize] == 1 { -v } else { v };
        }
        let mean = cw.iter().map(|v| v.abs()).sum::<f32>() / N as f32;
        let scale = 12.0 / mean.max(1e-12);
        let mut l = [0i8; N];
        for (o, &v) in l.iter_mut().zip(&cw) {
            *o = phy::quantise(v, scale);
        }
        let Some(bits) = phy::decode(&mut self.dec, &l, K, 50) else {
            return;
        };
        let Some(c) = parse_control(&bits) else {
            return;
        };
        self.report.controls += 1;
        self.since_good = 0;
        for s in 0..4 {
            self.hint[s] = Profile::of(c.map[s]);
        }
        match c.kind {
            0 => {
                let mut bits = Vec::new();
                push_bytes(&mut bits, &c.bytes);
                if let Some(b) = beacon::parse(&bits[..beacon::HEADER_BITS]) {
                    out.push(Event::Beacon(b));
                }
            }
            1 => {
                let s = usize::from(c.bytes[0]);
                let call: Vec<u8> = c.bytes[1..].iter().copied().take_while(|&b| b != 0).collect();
                if s < 4 {
                    self.report.slots[s].1 = Some(String::from_utf8_lossy(&call).into_owned());
                }
            }
            2 => {
                if c.part == 0 {
                    self.ctrl_text = Some((Vec::new(), 0));
                }
                match self.ctrl_text.as_mut() {
                    Some((t, next)) if *next == c.part => {
                        t.extend_from_slice(&c.bytes);
                        *next += 1;
                        if *next == c.parts {
                            let mut t = std::mem::take(t);
                            while t.last() == Some(&0) {
                                t.pop();
                            }
                            out.push(Event::Text(t));
                            self.ctrl_text = None;
                        }
                    }
                    _ => self.ctrl_text = None,
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(bits: &[u8]) -> String {
        crate::family::pack(bits).iter().map(|b| format!("{b:02x}")).collect()
    }

    fn info_of(p: Profile) -> Vec<u8> {
        let id = p.id() as usize;
        let bytes: Vec<u8> = (0..40).map(|i| ((37 * i + id) % 256) as u8).collect();
        if p.voice() {
            let mut v = Vec::new();
            push_bytes(&mut v, &bytes);
            v.truncate(p.k());
            v
        } else if p == Profile::Idle {
            data_block(p, 0, 0, 5, &[])
        } else {
            data_block(p, 1, 0, 5, &bytes)
        }
    }

    #[test]
    fn chirp_and_pilots() {
        let c = chirp(1);
        assert!((c.re + 0.980785).abs() < 1e-5 && (c.im + 0.195090).abs() < 1e-5);
        let p = pilots();
        let signs = |s: usize| {
            (0..8)
                .map(|q| format!("{}{}", if p[s][q].re > 0.0 { '+' } else { '-' }, if p[s][q].im > 0.0 { '+' } else { '-' }))
                .collect::<Vec<_>>()
                .join(" ")
        };
        assert_eq!(signs(0), "+- -+ ++ +- +- -- +- -+");
        assert_eq!(signs(3), "-+ +- -+ -+ -+ -+ -+ +-");
    }

    #[test]
    fn slot_words_of_the_vectors() {
        let want = [
            "7bd3ca15524197fda74ca83a95cc1d718804f5f9350e2fea9a2ad14ae3237bfc88eb36d53250c54c69395dc64a1ebfc4356d206f037d49418bde425f834f758f6139775c",
            "ea9ade00abf6c5b810e31d141012b2aca5e77dfd03af8e5a2cb1b8af01134549b901e8b4afc33a6debb22c52a33285be763ce47434504027c1c5f582046007f6a392aa75",
            "ac97df80b9b68cfcd09d4565103328a4796416415003be54b61bcc9e07b851d06528b6977543924fbb21cf90ab2380db705c36380c7046d70ce431283b546e8c03a3e834",
            "caf45a687a05e82dcc654f9cd8a3ed3519fdb611614ebedc04619afe93eac270e068bc186c73118fe3eb6d0e2fff956d7e1e8e325d440e461101a1aea0c10cf023de6b7a",
            "2662f7c54ef2d748555c0b8db774450d9c6298c51af7460048360e1e1344086beb8b",
            "6ab1aa537df4b39d57d0938b10538f38ab473b59717f00a518cc9f20705d8e7294ff7b7a8bfe16a18268f9be7b26c90a093accc6025e7b5f9b605c86181320c2f27c9b4a",
            "b809ae0a5cd47efe29e7d3ef5d1ced501582658faeb75347b16cde17c686836b653f",
        ];
        let mut dec = phy::decoder();
        for (p, w) in Profile::ALL.iter().zip(want) {
            let info = info_of(*p);
            let bits = slot_bits(1, *p, &info);
            assert_eq!(hex(&bits), w, "{p:?}");
            // Round trip, and not in another slot.
            let llr: Vec<f32> = bits.iter().map(|&b| if b == 0 { 4.0 } else { -4.0 }).collect();
            assert_eq!(decode_slot(&mut dec, 1, *p, &llr, 40), Some(info.clone()), "{p:?}");
            assert_eq!(decode_slot(&mut dec, 2, *p, &llr, 40), None, "{p:?}");
            for q in Profile::ALL {
                if q != *p && q.k() != p.k() && q.n() == p.n() {
                    assert_eq!(decode_slot(&mut dec, 1, q, &llr, 20), None, "{p:?} as {q:?}");
                }
            }
        }
    }

    #[test]
    fn control_word_of_the_vector() {
        let c = Control {
            map: [2, 5, 5, 5],
            kind: 2,
            part: 1,
            parts: 3,
            bytes: std::array::from_fn(|i| b'A' + (i % 26) as u8),
        };
        assert_eq!(
            hex(&control_sent(&c)),
            "25c203721fd4b608c82980e784a6ca6412de1f6c59fa1d8f3dd112e576cf5402cf0a13c389fe38a4216d549773ff8da7e5214f77e96b39877c6ecef218f335ea8c7bf550025bf22683fc00ce1211f103"
        );
        assert_eq!(parse_control(&control_bits(&c)), Some(c));
    }
}
