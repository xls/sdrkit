//! Encoders: message text to channel symbols and to audio, for every mode, and the sync patterns the decoders share.

use crate::mode::FskSpec;
use crate::mode::{Mode, Sub};
use crate::proto::codes::{self, CodeId, CodeMissing, CodeStore};
use crate::proto::js8msg;
use crate::proto::msg77::{pack77, PackError, RVEC};
use crate::proto::{jt65, k32, msg72, wsprmsg};
use num_complex::Complex32;
use sdrkit_algo_fec::ldpc::LdpcCode;
use sdrkit_algo_fec::nbldpc::NbCode;
use sdrkit_util_bits::crc::cyclic::{FST4_CRC24, FT8_CRC14, MSK144_CRC13, Q65_CRC12};
use sdrkit_util_bits::pack::push_u64;

/// Why a message is not encoded.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum EncodeError {
    /// The text does not fit the mode's message.
    #[error(transparent)]
    Pack(#[from] PackError),
    /// The mode's code is not available.
    #[error(transparent)]
    NoCode(#[from] CodeMissing),
}

/// The `n` code bits of `info` with `code` (empty when the lengths do not fit).
fn encode_with(code: &LdpcCode, info: &[u8]) -> Vec<u8> {
    let mut cw = vec![0u8; code.n()];
    if code.encode(info, &mut cw).is_err() {
        cw.clear();
    }
    cw
}

/// The loaded code `id`, or the reason it is missing.
fn loaded(codes: &CodeStore, id: CodeId) -> Result<&LdpcCode, EncodeError> {
    codes
        .get(id)
        .map(|c| &**c)
        .ok_or(EncodeError::NoCode(CodeMissing::Table(id)))
}

/// The FT8 Costas array.
pub const COSTAS8: [u8; 7] = [3, 1, 4, 0, 6, 5, 2];
/// The FT4 / FT2 sync arrays.
pub const SYNC4: [[u8; 4]; 4] = [[0, 1, 3, 2], [1, 0, 2, 3], [2, 3, 1, 0], [3, 2, 0, 1]];
/// The Gray map of 8-FSK data symbols.
pub const GRAY8: [u8; 8] = [0, 1, 3, 2, 5, 6, 4, 7];
/// The Gray map of 4-FSK data symbols.
pub const GRAY4: [u8; 4] = [0, 1, 3, 2];
/// The 162-bit WSPR sync vector.
pub const WSPR_SYNC: &str = "110000001000111000100101111000000010010100000010110011010001101000011010101010010010110001101010001000001001001110110011010001110000010100110000000110101100011000";
/// The 206-bit JT4 sync vector.
pub const JT4_SYNC: &str = "00011000110110010100000001100000000000010110110101111101000100100111110001010001111011001000110101010101111101010110101011100101101111000011011000111011101110010001101100100011111100110000110001011011110101";
/// The 126-bit JT65 sync vector (1: tone 0).
pub const JT65_SYNC: &str = "100110001111110101000101100100011100111101101111000110101011001101010100100000011000000011010010110101010011001001000011111111";
/// The JT9 sync symbol positions.
pub const JT9_SYNC: [usize; 16] = [0, 1, 4, 9, 15, 22, 32, 34, 50, 51, 54, 59, 65, 72, 82, 84];
/// The Q65 sync symbol positions (0-based).
pub const Q65_SYNC: [usize; 22] = [
    0, 8, 11, 12, 14, 21, 22, 25, 26, 32, 34, 37, 45, 49, 54, 59, 61, 65, 68, 73, 75, 84,
];
/// The FST4 sync words: words 1, 3, 5 and words 2, 4.
pub const FST4_SYNC: [[u8; 8]; 2] = [[0, 1, 3, 2, 1, 0, 2, 3], [2, 3, 1, 0, 3, 2, 0, 1]];
/// The MSK144 sync word.
pub const MSK_SYNC: [u8; 8] = [0, 1, 1, 1, 0, 0, 1, 0];
/// The JS8 Costas arrays of Normal (all three the same) and of Fast and Turbo.
pub const JS8_COSTAS: [[[u8; 7]; 3]; 2] = [
    [
        [4, 2, 5, 6, 1, 3, 0],
        [4, 2, 5, 6, 1, 3, 0],
        [4, 2, 5, 6, 1, 3, 0],
    ],
    [
        [0, 6, 2, 3, 5, 4, 1],
        [1, 5, 0, 2, 3, 6, 4],
        [2, 5, 0, 6, 4, 1, 3],
    ],
];

/// A sync vector as bits.
pub fn sync_bits(s: &str) -> Vec<u8> {
    s.bytes().map(|b| b - b'0').collect()
}

/// The JS8 Costas arrays of a sub-mode.
pub fn js8_costas(sub: u8) -> &'static [[u8; 7]; 3] {
    &JS8_COSTAS[(sub > 0) as usize]
}

/// The 174 code bits of FT8 / FT4 / FT2 from 77 message bits (`scramble`: FT4 / FT2).
pub fn ft_codeword(bits77: &[u8], scramble: bool) -> Vec<u8> {
    let mut m: Vec<u8> = bits77.to_vec();
    if scramble {
        for (b, r) in m.iter_mut().zip(RVEC.iter()) {
            *b ^= r;
        }
    }
    let c = FT8_CRC14.compute(&m);
    push_u64(&mut m, c, 14);
    encode_with(codes::ft8(), &m)
}

/// The 79 FT8 tones of 174 code bits.
pub fn ft8_tones(cw: &[u8]) -> Vec<u8> {
    let mut t = Vec::with_capacity(79);
    let data: Vec<u8> = cw
        .chunks(3)
        .map(|c| GRAY8[(c[0] * 4 + c[1] * 2 + c[2]) as usize])
        .collect();
    t.extend(COSTAS8);
    t.extend(&data[..29]);
    t.extend(COSTAS8);
    t.extend(&data[29..]);
    t.extend(COSTAS8);
    t
}

/// The 103 FT4 / FT2 tones of 174 code bits (without the ramp symbols).
pub fn ft4_tones(cw: &[u8]) -> Vec<u8> {
    let data: Vec<u8> = cw
        .chunks(2)
        .map(|c| GRAY4[(c[0] * 2 + c[1]) as usize])
        .collect();
    let mut t = Vec::with_capacity(103);
    t.extend(SYNC4[0]);
    t.extend(&data[..29]);
    t.extend(SYNC4[1]);
    t.extend(&data[29..58]);
    t.extend(SYNC4[2]);
    t.extend(&data[58..]);
    t.extend(SYNC4[3]);
    t
}

fn wspr_symbols(bits50: &[u8]) -> Vec<u8> {
    let cw = k32::encode(bits50);
    let sync = sync_bits(WSPR_SYNC);
    cw.iter().zip(sync).map(|(&d, s)| s + 2 * d).collect()
}

fn gray(x: u8) -> u8 {
    x ^ (x >> 1)
}

fn jt65_symbols(dat: &[u8; 12]) -> Vec<u8> {
    let cw = jt65::interleave(&jt65::encode(dat));
    let sync = sync_bits(JT65_SYNC);
    let mut k = 0;
    sync.iter()
        .map(|&s| {
            if s == 1 {
                0
            } else {
                k += 1;
                2 + gray(cw[k - 1])
            }
        })
        .collect()
}

fn jt9_symbols(dat: &[u8; 12]) -> Vec<u8> {
    let mut cw = k32::encode(&msg72::bits72(dat));
    cw.push(0);
    let data: Vec<u8> = cw
        .chunks(3)
        .map(|c| 1 + gray(c[0] * 4 + c[1] * 2 + c[2]))
        .collect();
    let mut out = vec![0u8; 85];
    let mut k = 0;
    for (i, o) in out.iter_mut().enumerate() {
        if !JT9_SYNC.contains(&i) {
            *o = data[k];
            k += 1;
        }
    }
    out
}

fn jt4_symbols(dat: &[u8; 12]) -> Vec<u8> {
    let cw = k32::encode(&msg72::bits72(dat));
    let sync = sync_bits(JT4_SYNC);
    cw.iter().zip(sync).map(|(&d, s)| s + 2 * d).collect()
}

/// The two CRC symbols of 13 Q65 message symbols. The CRC-12 runs over the symbols in order, each least significant bit
/// first. Its 12 bits, highest power first, fill the two symbols least significant bit first.
pub fn q65_crc(msg: &[u8; 13]) -> [u8; 2] {
    let bits: Vec<u8> = msg
        .iter()
        .flat_map(|&x| (0..6).map(move |k| x >> k & 1))
        .collect();
    let c = Q65_CRC12.compute(&bits);
    let part = |hi: u32| (0..6).fold(0u8, |v, i| v | ((c >> (hi - i) & 1) as u8) << i);
    [part(11), part(5)]
}

/// The 15 information symbols of the Q65 code: the 77 message bits and a zero as 13 six-bit symbols, most significant bit
/// first, then the two CRC symbols ([`q65_crc`]).
pub fn q65_info(bits77: &[u8]) -> [u8; 15] {
    let mut x = [0u8; 15];
    for (i, &b) in bits77.iter().take(77).enumerate() {
        x[i / 6] |= (b & 1) << (5 - i % 6);
    }
    let crc = q65_crc(x[..13].try_into().unwrap_or(&[0; 13]));
    x[13..].copy_from_slice(&crc);
    x
}

/// The 63 Q65 channel symbols of 15 information symbols with the loaded code: the 13 message symbols, then the 50 check
/// symbols. The two CRC symbols are not sent. Empty when the code does not fit.
pub fn q65_channel(code: &NbCode, info: &[u8; 15]) -> Vec<u8> {
    let x: Vec<u16> = info.iter().map(|&v| u16::from(v)).collect();
    let mut word = [0u16; 65];
    if code.n() != 65 || code.encode(&x, &mut word).is_err() {
        return Vec::new();
    }
    word[..13]
        .iter()
        .chain(&word[15..])
        .map(|&v| v as u8)
        .collect()
}

/// The 85 Q65 tones of 63 channel symbols: tone 0 at the sync positions, else the channel symbol plus 1.
pub fn q65_tones(channel: &[u8]) -> Vec<u8> {
    let mut data = channel.iter();
    (0..85)
        .map(|i| {
            if Q65_SYNC.contains(&i) {
                0
            } else {
                data.next().map_or(0, |&c| c + 1)
            }
        })
        .collect()
}

/// The FST4 / FST4W information bits (FST4: the scrambled message and CRC-24; FST4W: the 50 bits and CRC-24).
pub fn fst4_info(bits: &[u8], wspr: bool) -> Vec<u8> {
    let mut m = bits.to_vec();
    if !wspr {
        for (b, r) in m.iter_mut().zip(RVEC.iter()) {
            *b ^= r;
        }
    }
    let c = FST4_CRC24.compute(&m);
    push_u64(&mut m, c, 24);
    m
}

/// The 160 FST4 tones of 240 code bits.
pub fn fst4_tones(cw: &[u8]) -> Vec<u8> {
    let data: Vec<u8> = cw
        .chunks(2)
        .map(|c| GRAY4[(c[0] * 2 + c[1]) as usize])
        .collect();
    let mut t = Vec::with_capacity(160);
    for w in 0..4 {
        t.extend(FST4_SYNC[w % 2]);
        t.extend(&data[30 * w..30 * w + 30]);
    }
    t.extend(FST4_SYNC[0]);
    t
}

/// The 90 information bits of MSK144: the 77 message bits and their CRC-13.
pub fn msk144_info(bits77: &[u8]) -> Vec<u8> {
    let mut m = bits77.to_vec();
    push_u64(&mut m, MSK144_CRC13.compute(bits77), 13);
    m
}

/// The 144 MSK144 channel bits of 77 message bits, with the loaded (128,90) code.
pub fn msk144_bits(code: &LdpcCode, bits77: &[u8]) -> Vec<u8> {
    let cw = encode_with(code, &msk144_info(bits77));
    if cw.len() != 128 {
        return Vec::new();
    }
    let mut f = Vec::with_capacity(144);
    f.extend(MSK_SYNC);
    f.extend(&cw[..48]);
    f.extend(MSK_SYNC);
    f.extend(&cw[48..]);
    f
}

/// The 79 JS8 tones of a frame (72 bits) and its flags.
pub fn js8_tones(sub: u8, frame72: &[u8], flags: u8) -> Vec<u8> {
    let info = js8msg::info87(frame72, flags);
    // The code word is in transmission order (information bits last); three bits per tone, no Gray code.
    let cw = encode_with(codes::js8(), &info);
    if cw.len() != 174 {
        return Vec::new();
    }
    let data: Vec<u8> = cw.chunks(3).map(|c| c[0] * 4 + c[1] * 2 + c[2]).collect();
    let cs = js8_costas(sub);
    let mut t = Vec::with_capacity(79);
    t.extend(cs[0]);
    t.extend(&data[..29]);
    t.extend(cs[1]);
    t.extend(&data[29..]);
    t.extend(cs[2]);
    t
}

/// The channel symbols of a message as the encoder vectors list them: FT8 79 tones, FT4 / FT2 103 tones (no ramp symbols), WSPR
/// 162, JT65 126, JT9 85, JT4 206, FST4 / FST4W 160, MSK144 the 144 frame bits. FST4, FST4W and MSK144 take their code from
/// `codes`, and so does Q65 (85 tones). (JS8 frames: [`js8_tones`].)
pub fn channel_symbols(sub: &Sub, text: &str, codes: &CodeStore) -> Result<Vec<u8>, EncodeError> {
    Ok(match sub.mode {
        Mode::Ft8 => ft8_tones(&ft_codeword(&pack77(text)?, false)),
        Mode::Ft4 | Mode::Ft2 => ft4_tones(&ft_codeword(&pack77(text)?, true)),
        Mode::Wspr => wspr_symbols(&wsprmsg::pack50(text)?),
        Mode::Jt65 => jt65_symbols(&msg72::pack72(text)?),
        Mode::Jt9 => jt9_symbols(&msg72::pack72(text)?),
        Mode::Jt4 => jt4_symbols(&msg72::pack72(text)?),
        Mode::Q65 => {
            let info = q65_info(&pack77(text)?);
            let code = codes
                .symbols(CodeId::Q65)
                .ok_or(EncodeError::NoCode(CodeMissing::Table(CodeId::Q65)))?;
            q65_tones(&q65_channel(code, &info))
        }
        Mode::Fst4 => {
            let info = fst4_info(&pack77(text)?, false);
            fst4_tones(&encode_with(loaded(codes, CodeId::Fst4)?, &info))
        }
        Mode::Fst4w => {
            let info = fst4_info(&wsprmsg::pack50(text)?, true);
            fst4_tones(&encode_with(loaded(codes, CodeId::Fst4w)?, &info))
        }
        Mode::Msk144 => msk144_bits(loaded(codes, CodeId::Msk144)?, &pack77(text)?),
        Mode::Js8 => {
            return Err(PackError::Js8Frames.into());
        }
    })
}

/// The tones actually sent (FT4 / FT2 with their ramp symbols; MSK144 the tones of one frame).
pub fn frame_tones(sub: &Sub, symbols: &[u8]) -> Vec<u8> {
    match sub.mode {
        Mode::Ft4 | Mode::Ft2 => {
            let mut t = Vec::with_capacity(105);
            t.push(symbols[0]);
            t.extend_from_slice(symbols);
            t.push(symbols[symbols.len() - 1]);
            t
        }
        Mode::Msk144 => msk_tones(symbols),
        _ => symbols.to_vec(),
    }
}

/// Whether a mode is offered to the host for transmission: FT8, FT4, FT2, WSPR, JT65, JT9, JT4 and JS8.
pub fn offered_for_transmit(mode: Mode) -> bool {
    matches!(
        mode,
        Mode::Ft8
            | Mode::Ft4
            | Mode::Ft2
            | Mode::Wspr
            | Mode::Jt65
            | Mode::Jt9
            | Mode::Jt4
            | Mode::Js8
    )
}

/// What to make audio of.
#[derive(Clone, Debug)]
pub struct Transmission {
    /// The mode.
    pub sub: Sub,
    /// The audio frequency of tone 0 (MSK144: the carrier), Hz.
    pub hz: f64,
    /// Peak amplitude (full scale 1.0).
    pub amp: f32,
    /// Sent upside down (tones running downwards; FT8, FT4, FT2, WSPR).
    pub mirrored: bool,
}

/// The audio of a symbol sequence (from [`channel_symbols`] or [`js8_tones`]) at `rate`; MSK144 repeats its frame to fill
/// `msk_s` seconds. The audio is the imaginary part of [`baseband_of`].
pub fn audio_of(t: &Transmission, symbols: &[u8], rate: f64, msk_s: f64, out: &mut Vec<f32>) {
    let mut z = Vec::new();
    baseband_of(t, symbols, rate, msk_s, &mut z);
    out.extend(z.iter().map(|v| v.im));
}

/// The complex baseband of a symbol sequence at `rate`, with amplitude `t.amp`: the upper sideband around a suppressed
/// carrier at 0 Hz, tone 0 at `t.hz`. MSK144 repeats its frame to fill `msk_s` seconds.
pub fn baseband_of(
    t: &Transmission,
    symbols: &[u8],
    rate: f64,
    msk_s: f64,
    out: &mut Vec<Complex32>,
) {
    let mut spec = t.sub.fsk();
    let tones = frame_tones(&t.sub, symbols);
    let start = out.len();
    if t.sub.mode == Mode::Msk144 {
        // Continuous phase over the repeated frames: the frame's tone sequence repeated.
        let frames = ((msk_s / t.sub.tx_s()).ceil() as usize).max(1);
        let mut all = Vec::with_capacity(frames * tones.len());
        for _ in 0..frames {
            all.extend_from_slice(&tones);
        }
        waveform(&spec, &all, t.hz - 500.0, rate, out);
        out.truncate(start + (msk_s * rate).round() as usize);
    } else {
        if t.mirrored {
            spec.spacing_hz = -spec.spacing_hz;
        }
        waveform(&spec, &tones, t.hz, rate, out);
    }
    for v in &mut out[start..] {
        *v *= t.amp;
    }
}

/// Appends the complex waveform of `tones` at `rate` to `out`: tone 0 at `f0_hz`, unit amplitude, with the ramps, the first
/// sample at the start of the first symbol. The transmitter, the test signal and the subtraction of decoded signals all use
/// this function, so they make the same waveform. A spec out of range produces no samples.
pub fn waveform(spec: &FskSpec, tones: &[u8], f0_hz: f64, rate: f64, out: &mut Vec<Complex32>) {
    let Ok(mut m) = spec.modulator(rate) else {
        return;
    };
    let levels: Vec<f32> = tones.iter().map(|&t| t as f32).collect();
    let start = out.len();
    m.push(&levels, out);
    m.flush(out);
    let w = std::f64::consts::TAU * f0_hz / rate;
    for (n, v) in out[start..].iter_mut().enumerate() {
        let (s, c) = (w * n as f64).sin_cos();
        *v *= Complex32::new(c as f32, s as f32);
    }
}

/// The MSK144 tone sequence of 144 channel bits: tone 0 is the carrier - 500 Hz, tone 1 the carrier + 500 Hz, two tones per
/// millisecond.
pub fn msk_tones(bits: &[u8]) -> Vec<u8> {
    let n = bits.len();
    let s = |i: usize| 2 * bits[i % n] as i32 - 1;
    let mut t = vec![0u8; n];
    for i in 0..n / 2 {
        t[2 * i] = ((s(2 * i + 1) * s(2 * i) + 1) / 2) as u8;
        t[2 * i + 1] = (-(s(2 * i + 1) * s(2 * i + 2) - 1) / 2) as u8;
    }
    t
}

/// The audio of a message: refuses a text that cannot be packed, or a mode whose code is missing (no audio is made).
pub fn audio(
    t: &Transmission,
    text: &str,
    rate: f64,
    codes: &CodeStore,
) -> Result<Vec<f32>, EncodeError> {
    let s = channel_symbols(&t.sub, text, codes)?;
    let mut out = Vec::new();
    audio_of(t, &s, rate, t.sub.period - 0.1, &mut out);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tone_lands_where_it_should() {
        let spec = FskSpec {
            symbol_s: 0.16,
            spacing_hz: 6.25,
            bt: 2.0,
            ramp: crate::mode::Ramp::Edge(0.125),
        };
        let mut z = Vec::new();
        waveform(&spec, &[3; 20], 1000.0, 8000.0, &mut z);
        let out: Vec<f32> = z.iter().map(|v| v.im).collect();
        assert_eq!(out.len(), 25600);
        // Correlate with a tone at 1000 + 3 x 6.25 Hz.
        let f = 1018.75;
        let (mut re, mut im) = (0.0, 0.0);
        for (i, &x) in out.iter().enumerate() {
            let p = std::f64::consts::TAU * f * i as f64 / 8000.0;
            re += x as f64 * p.cos();
            im += x as f64 * p.sin();
        }
        let a = 2.0 * (re * re + im * im).sqrt() / out.len() as f64;
        assert!(a > 0.95, "{a}");
    }

    #[test]
    fn sync_vectors_have_their_lengths() {
        assert_eq!(WSPR_SYNC.len(), 162);
        assert_eq!(JT4_SYNC.len(), 206);
        assert_eq!(JT65_SYNC.len(), 126);
        assert_eq!(sync_bits(JT65_SYNC).iter().filter(|&&b| b == 1).count(), 63);
    }

    #[test]
    fn every_mode_with_a_code_encodes() {
        for m in Mode::ALL {
            if matches!(
                m,
                Mode::Js8 | Mode::Q65 | Mode::Fst4 | Mode::Fst4w | Mode::Msk144
            ) {
                continue;
            }
            let text = match m {
                Mode::Wspr => "K1ABC FN42 37",
                _ => "CQ K1ABC FN42",
            };
            let sub = Sub::new(m);
            let s = channel_symbols(&sub, text, &CodeStore::default()).unwrap();
            let t = Transmission {
                sub,
                hz: 1500.0,
                amp: 0.5,
                mirrored: false,
            };
            let mut out = Vec::new();
            audio_of(&t, &s, 8000.0, 1.0, &mut out);
            assert!(!out.is_empty(), "{m:?}");
            assert!(out.iter().all(|x| x.abs() <= 0.5001));
        }
    }
}
