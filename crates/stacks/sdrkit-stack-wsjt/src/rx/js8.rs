//! JS8 decoding: Costas sync over the band and the DT range, the fit and tone measurements of FT8, soft decisions,
//! belief propagation of the (174,87) code and the CRC-12.

use super::ft::{self, Layout};
use super::slot::*;
use crate::encode;
use crate::mode::{Mode, Sub};
use crate::proto::codes;
use crate::proto::js8msg::{self, Frame};
use num_complex::Complex32;

/// The type of a JS8 frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Js8FrameKind {
    /// A heartbeat.
    Heartbeat,
    /// A CQ (a heartbeat to all).
    Cq,
    /// A call with a locator or a command.
    Compound,
    /// A message from one station to another.
    Directed,
    /// Free text.
    Data,
}

/// One JS8 frame decoded.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Js8Frame {
    /// Slot start, UTC seconds.
    pub slot: f64,
    /// Sub-mode: 0 Normal, 1 Fast, 2 Turbo.
    pub submode: u8,
    /// Tone 0, Hz.
    pub hz: f64,
    /// SNR in 2500 Hz.
    pub snr: i32,
    /// Start against the nominal start, s.
    pub dt: f64,
    /// The 12 characters of the frame.
    pub chars: String,
    /// The flags (1 first, 2 last, 4 fast data).
    pub flags: u8,
    /// The frame type.
    pub kind: Js8FrameKind,
    /// The display text.
    pub text: String,
    /// From (a call, empty for data).
    pub from: String,
    /// To (a call or group).
    pub to: String,
    /// Locator.
    pub grid: String,
    /// Command text (directed frames).
    pub cmd: String,
    /// The pass.
    pub pass: u32,
    /// The dial frequency, Hz.
    pub dial: f64,
}

/// The layout of a JS8 sub-mode.
pub fn layout(sub: &Sub) -> Layout {
    let cs = encode::js8_costas(sub.sub);
    let mut sync = Vec::new();
    for (w, b) in [0usize, 36, 72].into_iter().enumerate() {
        for (i, &t) in cs[w].iter().enumerate() {
            sync.push((b + i, t));
        }
    }
    let nss = [1280, 800, 400][sub.sub.min(2) as usize];
    Layout {
        nss,
        m: 8,
        bits: 3,
        nsym: 79,
        sync,
        data: (7..36).chain(43..72).collect(),
        label: (0..8).collect(),
        decim: [40, 25, 20][sub.sub.min(2) as usize],
        max_cand: 100,
        passes: 2,
        groups: [2, 3],
        spacing: sub.spacing_hz(),
    }
}

/// Decodes one slot of one JS8 sub-mode. `failed` counts the candidates that did not decode.
pub fn decode(slot: &Slot, failed: &mut u64) -> Vec<Js8Frame> {
    let sub = slot.sub;
    debug_assert_eq!(sub.mode, Mode::Js8);
    let lay = layout(&sub);
    let mut audio: Vec<f32> = slot
        .audio
        .iter()
        .map(|&x| if x.is_finite() { x } else { 0.0 })
        .collect();
    let fs = slot.rate / lay.decim as f64;
    let m_len = smooth_at_least(audio.len().div_ceil(lay.decim));
    let n_fft = m_len * lay.decim;
    let span = 7.0 * lay.spacing;
    let mut out: Vec<Js8Frame> = Vec::new();
    let mut made: Vec<(f64, f64)> = Vec::new();
    let mut s = LdpcDecoders::new(codes::js8());
    let mut cw = vec![0u8; 174];
    let mut bb = Vec::new();
    let mut c: Vec<Complex32> = Vec::new();
    for pass in 0..lay.passes {
        let sp = Spectrogram::new(&audio, lay.nss, lay.nss / 4, 2 * lay.nss, false, slot.rate);
        let spec = SlotSpectrum::new(&audio, n_fft);
        let mut found = 0;
        let top = 3200.0 - span;
        for cand in ft::candidates(&sp, &lay, slot, false, 200.0, top, lay.max_cand) {
            let f0 = cand.bin as f64 * sp.df;
            let t0 = slot.time_of((cand.row * sp.step as i64) as f64);
            if made
                .iter()
                .any(|&(f, t)| (f - f0).abs() < 2.0 * lay.spacing && (t - t0).abs() < 0.5)
            {
                continue;
            }
            let shift = spec.baseband(
                f0,
                f0 - 1.5 * lay.spacing,
                f0 + span + 1.5 * lay.spacing,
                1.5 * lay.spacing,
                m_len,
                &mut bb,
            );
            let i_guess = ((cand.row * sp.step as i64) as f64 / lay.decim as f64).round() as i64;
            let fit = ft::fit(&bb, &lay, &lay.sync, 1.0, fs, i_guess, f0 - shift);
            ft::tone_amplitudes(&bb, &lay, 1.0, fs, &fit, &mut c);
            let amps: Vec<f32> = lay
                .data
                .iter()
                .flat_map(|&k| (0..8).map(move |t| (k, t)))
                .map(|(k, t)| c[k * 8 + t].norm())
                .collect();
            // The code word is in transmission order: 87 parity bits, then the 87 information bits.
            let mut llr = Vec::with_capacity(174);
            fsk_llrs(&amps, 8, 3, &lay.label, &mut llr);
            let checked = (llr.len() == 174)
                .then(|| s.decode(&llr, 40, false, &mut cw))
                .flatten()
                .and_then(|_| js8msg::check87(&cw[87..]));
            let Some((bits72, flags)) = checked else {
                *failed += 1;
                continue;
            };
            let Some(frame) = Frame::unpack(&bits72).filter(plausible) else {
                *failed += 1;
                continue;
            };
            // The tones as received, for the SNR and subtraction: the information tones re-encoded, the parity tones as heard.
            let tones = encode::js8_tones(sub.sub, &bits72, flags);
            let snr = ft::snr_of(&c, &lay, &tones);
            let hz = shift + fit.df;
            let start = (fit.i0 as f64 + fit.frac) * lay.decim as f64;
            let t_tx = slot.time_of(start);
            let r = clean_copy(&sub, &tones, hz, false, slot.rate);
            subtract(&mut audio, start.round() as i64, &r, lay.nss);
            found += 1;
            made.push((hz, t_tx));
            let chars = js8msg::bits_to_chars(&bits72);
            if out.iter().any(|o| o.chars == chars && o.flags == flags) {
                continue;
            }
            out.push(frame_record(
                slot,
                &frame,
                chars,
                flags,
                hz,
                snr,
                t_tx - sub.t0(),
                pass as u32,
            ));
        }
        if found == 0 {
            break;
        }
    }
    out.sort_by(|a, b| a.hz.total_cmp(&b.hz));
    out
}

/// A frame whose numbers are in range: SNR reports within +-30 dB.
fn plausible(f: &Frame) -> bool {
    match f {
        Frame::Directed { cmd, num, .. }
        | Frame::Compound {
            cmd: Some(cmd),
            num,
            ..
        } => {
            let c = js8msg::COMMANDS[*cmd as usize];
            !(c == " SNR" || c == " HEARTBEAT SNR") || (-30..=30).contains(num)
        }
        _ => true,
    }
}

/// The record of a decoded frame.
#[allow(clippy::too_many_arguments)]
pub fn frame_record(
    slot: &Slot,
    f: &Frame,
    chars: String,
    flags: u8,
    hz: f64,
    snr: f64,
    dt: f64,
    pass: u32,
) -> Js8Frame {
    let (kind, from, to, grid, cmd) = match f {
        Frame::Heartbeat { call, grid, cq, .. } => (
            if *cq {
                Js8FrameKind::Cq
            } else {
                Js8FrameKind::Heartbeat
            },
            call.clone(),
            if *cq {
                "@ALLCALL".to_string()
            } else {
                "@HB".to_string()
            },
            grid.clone(),
            String::new(),
        ),
        Frame::Compound {
            call, grid, cmd, ..
        } => (
            Js8FrameKind::Compound,
            call.clone(),
            String::new(),
            grid.clone(),
            cmd.map(|c| js8msg::COMMANDS[c as usize].to_string())
                .unwrap_or_default(),
        ),
        Frame::Directed { from, to, cmd, .. } => (
            Js8FrameKind::Directed,
            from.clone(),
            to.clone(),
            String::new(),
            js8msg::COMMANDS[*cmd as usize].to_string(),
        ),
        Frame::Data { .. } => (
            Js8FrameKind::Data,
            String::new(),
            String::new(),
            String::new(),
            String::new(),
        ),
    };
    Js8Frame {
        slot: slot.utc,
        submode: slot.sub.sub,
        hz,
        snr: (snr.round() as i32).max(-35),
        dt,
        chars,
        flags,
        kind,
        text: f.display(),
        from,
        to,
        grid,
        cmd,
        pass,
        dial: slot.dial_hz,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::encode::{audio_of, js8_tones, Transmission};
    use crate::rx::testutil::noise;
    use std::sync::Arc;

    #[test]
    fn js8_frames_decode_in_every_submode() {
        for sm in 0..3u8 {
            let sub = Sub::new(Mode::Js8).with_sub(sm);
            let (w0, w1) = sub.window();
            let n = ((w1 - w0) * 8000.0) as usize;
            let mut a = noise(n, 0.05, 9 + sm as u64);
            let f = Frame::Heartbeat {
                call: "LY2ABC".into(),
                grid: "KO24".into(),
                cq: false,
                choice: 0,
            };
            let tones = js8_tones(sm, &f.pack().unwrap(), 3);
            let amp = (2.0 * 10f64.powf(-12.0 / 10.0) * 0.0025 * 2500.0 / 4000.0).sqrt() as f32;
            let mut s = Vec::new();
            audio_of(
                &Transmission {
                    sub,
                    hz: 1000.0,
                    amp,
                    mirrored: false,
                },
                &tones,
                8000.0,
                0.0,
                &mut s,
            );
            let start = ((sub.t0() + 0.3 - w0) * 8000.0) as usize;
            for (i, v) in s.iter().enumerate() {
                a[start + i] += v;
            }
            let slot = Slot {
                sub,
                utc: 0.0,
                audio: Arc::new(a),
                rate: 8000.0,
                t_first: w0,
                search: false,
                dial_hz: 0.0,
                index: 0,
                early: false,
                seq: 0,
            };
            let mut failed = 0;
            let d = decode(&slot, &mut failed);
            assert_eq!(d.len(), 1, "sub-mode {sm}");
            assert_eq!(d[0].text, "LY2ABC: @HB HEARTBEAT KO24");
            assert!((d[0].hz - 1000.0).abs() < 1.0);
            assert!((d[0].dt - 0.3).abs() < 0.05, "{}", d[0].dt);
            assert!((d[0].snr + 12).abs() <= 3, "{}", d[0].snr);
        }
    }
}
