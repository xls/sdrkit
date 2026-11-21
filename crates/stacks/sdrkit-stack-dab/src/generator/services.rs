//! The generator's services: each one's logical frames for the 12 s loop (DAB+ superframes with AAC-LC and PAD, MPEG Layer II
//! frames with their ScF-CRC and F-PAD, or data).

use super::content::{render, Programme, LOOP_S};
use super::data::{mot_group, mot_header};
use crate::audio::dabplus::AudioParams;
use crate::data::pad::write as padw;
use crate::fec::crc_of;
use sdrkit_algo_fec::rs::{codes::DAB_PLUS_120_110, Rs};
use sdrkit_codec_aac::{Encoder as AacEncoder, FRAME as N};
use sdrkit_codec_mpa::Encoder as Mp2Encoder;
use sdrkit_proto_bcdata::mp2;
use sdrkit_util_bits::crc::catalog::CRC16_DAB_FIRE;

/// Logical frames per loop.
pub const LOOP_FRAMES: usize = 500;

/// The PAD a DAB+ service carries: a label and a slide.
#[derive(Clone, Debug, Default)]
pub struct PadContent {
    /// The dynamic label (EBU Latin).
    pub label: Option<String>,
    /// The slide: (name, PNG bytes).
    pub slide: Option<(String, Vec<u8>)>,
    /// A second slide sent after the first under the same transport id, as stations that reuse one id do: (name, PNG bytes).
    pub next_slide: Option<(String, Vec<u8>)>,
    /// DL Plus tags for the label: (content type, start, length − 1).
    pub dl_plus: Vec<(u8, u8, u8)>,
}

/// The X-PAD of each access unit in turn (variable size, with content indicators, logical order), cycling.
pub fn pad_cycle(p: &PadContent) -> Vec<Vec<u8>> {
    let mut cycle = Vec::new();
    if let Some(label) = &p.label {
        let bytes = crate::registry::to_ebu_latin(label);
        let (a, b) = bytes.split_at(bytes.len().min(16));
        for i in 0..8 {
            let toggle = false;
            let seg = if i % 2 == 0 || b.is_empty() {
                padw::dl_segment(toggle, true, b.is_empty(), 0, 0, a)
            } else {
                padw::dl_segment(toggle, false, true, 1, 0, b)
            };
            cycle.push(padw::variable_xpad(&[(2, seg)]));
        }
        if !p.dl_plus.is_empty() {
            cycle.push(padw::variable_xpad(&[(
                2,
                padw::dl_plus(false, false, false, true, &p.dl_plus),
            )]));
        }
    }
    for (name, png) in p.slide.iter().chain(p.next_slide.iter()) {
        let header = mot_header(png.len() as u32, 2, 3, name);
        let gh = mot_group(3, 0x1234, &header);
        let gb = mot_group(4, 0x1234, png);
        for g in [gh, gb] {
            let mut first = true;
            for piece in g.chunks(32) {
                let mut sub = piece.to_vec();
                sub.resize(32, 0);
                if first {
                    cycle.push(padw::variable_xpad(&[
                        (1, padw::length_indicator(g.len())),
                        (12, sub),
                    ]));
                    first = false;
                } else {
                    cycle.push(padw::variable_xpad(&[(13, sub)]));
                }
            }
        }
    }
    cycle
}

/// The logical frames of a DAB+ service for the loop (each `bitrate × 3` bytes).
pub fn dabplus_frames(
    bitrate: u16,
    rate: u32,
    programme: Programme,
    pad: &PadContent,
) -> Vec<Vec<u8>> {
    let n = bitrate as usize / 8;
    let header: u8 = if rate == 48_000 { 0x50 } else { 0x10 };
    let params = AudioParams::from_header(header).expect("valid header");
    let units = params.units();
    let first = 3 + (12 * (units - 1)).div_ceil(8);
    let share = (110 * n - first) / units;
    let mut sizes = vec![share; units];
    sizes[units - 1] = 110 * n - first - share * (units - 1);
    let left = render(programme, false, rate);
    let right = render(programme, true, rate);
    let aus_per_loop = (LOOP_S * rate as f64 / N as f64).round() as usize;
    let cycle = pad_cycle(pad);
    // DAB+ without SBR runs at 48 or 32 kHz, the two rates of the encoder.
    let Ok(mut enc) = AacEncoder::new(rate) else {
        return Vec::new();
    };
    let mut aus = Vec::with_capacity(aus_per_loop);
    // Two passes, keeping the second, so the loop has no start-up transient.
    for pass in 0..2 {
        for a in 0..aus_per_loop {
            let size = sizes[a % units] - 2;
            let pad_bytes = if cycle.is_empty() {
                Vec::new()
            } else {
                padw::dse(&cycle[a % cycle.len()], padw::fpad(2, true))
            };
            let l = &left[a * N..(a + 1) * N];
            let r = &right[a * N..(a + 1) * N];
            let mut au = Vec::with_capacity(size);
            if enc.encode(l, r, &pad_bytes, size, &mut au).is_err() {
                // The PAD does not fit the unit: send it without PAD.
                if enc.encode(l, r, &[], size, &mut au).is_err() {
                    au = vec![0; size];
                }
            }
            if pass == 1 {
                aus.push(au);
            }
        }
    }
    let rs = Rs::new(DAB_PLUS_120_110).unwrap_or_else(|_| unreachable!("a table code"));
    let mut frames = Vec::with_capacity(LOOP_FRAMES);
    for sf in 0..LOOP_FRAMES / 5 {
        let mut p = vec![0u8; 110 * n];
        p[2] = header;
        let mut start = first;
        let mut starts = Vec::new();
        for k in 0..units {
            let au = &aus[sf * units + k];
            let end = start + au.len();
            p[start..end].copy_from_slice(au);
            let c = crc_of(&p[start..end]);
            p[end..end + 2].copy_from_slice(&c.to_be_bytes());
            starts.push(start);
            start = end + 2;
        }
        // The 12-bit start table of units 2 … n.
        let mut w_buf = Vec::new();
        let mut w = crate::bits::BitWriter::new(&mut w_buf);
        for &s in &starts[1..] {
            w.put(s as u64, 12);
        }
        let t = {
            let _ = w.finish();
            w_buf
        };
        p[3..3 + t.len()].copy_from_slice(&t);
        let f = CRC16_DAB_FIRE.checksum(&p[2..11]);
        p[0..2].copy_from_slice(&f.to_be_bytes());
        let mut s = vec![0u8; 120 * n];
        s[..110 * n].copy_from_slice(&p);
        let mut word = vec![0u8; 110];
        let mut par = vec![0u8; 10];
        for i in 0..n {
            for j in 0..110 {
                word[j] = s[i + j * n];
            }
            // A 110-byte word always fits the code.
            let _ = rs.encode(&word, &mut par);
            for j in 0..10 {
                s[i + (110 + j) * n] = par[j];
            }
        }
        for k in 0..5 {
            frames.push(s[k * 24 * n..(k + 1) * 24 * n].to_vec());
        }
    }
    frames
}

/// The logical frames of an MPEG Layer II service (48 or 24 kHz, stereo or mono) for the loop.
pub fn mp2_frames(bitrate: u16, rate: u32, stereo: bool, programme: Programme) -> Vec<Vec<u8>> {
    let left = render(programme, false, rate);
    let right = render(programme, true, rate);
    // The loop uses the Layer II rates of DAB, so the encoder accepts them.
    let Ok(mut enc) = Mp2Encoder::new(bitrate, rate, stereo) else {
        return Vec::new();
    };
    let h = *enc.header();
    let per_loop = (LOOP_S * rate as f64 / 1152.0).round() as usize;
    let reserve = 2 + mp2::scf_crc_len(&h);
    let mut encoded = Vec::with_capacity(per_loop);
    for pass in 0..2 {
        for f in 0..per_loop {
            let mut frame = Vec::new();
            let s = f * 1152..(f + 1) * 1152;
            if enc
                .encode(&left[s.clone()], &right[s], reserve, &mut frame)
                .is_err()
            {
                return Vec::new();
            }
            if pass == 1 {
                encoded.push((frame, enc.scale_factors().to_vec()));
            }
        }
    }
    // The ScF-CRC of frame n covers the scale factors of frame n + 1.
    let mut frames = Vec::with_capacity(LOOP_FRAMES);
    for f in 0..per_loop {
        let next = &encoded[(f + 1) % per_loop];
        let crc = mp2::scf_crc(&h, &next.1);
        let crc = &crc[..mp2::scf_crc_len(&h)];
        let mut fr = encoded[f].0.clone();
        let n = fr.len();
        let at = n - 2 - crc.len();
        fr[at..n - 2].copy_from_slice(crc);
        fr[n - 2] = 0;
        fr[n - 1] = 0;
        let lf = bitrate as usize * 3;
        for part in fr.chunks(lf) {
            frames.push(part.to_vec());
        }
    }
    frames
}
