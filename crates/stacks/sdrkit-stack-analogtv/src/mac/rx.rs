//! The components of the MAC receiver: the FM discriminator with the MAC de-emphasis, the line decoder, the packet demultiplexer
//! with the service identification and the sound, the picture builder and the display.
//!
//! Standard: ETS 300 250.

use super::codes::{dispersal, header_code, PACKETS, PACKET_BITS};
use super::frame::*;
use super::packet::{
    decode_group, decode_params, decode_sound, group_len, ids, text_of, Packet, DUMMY, NEXT,
    SI_ADDRESS, SI_FIRST,
};
use crate::results::{series_id, Aspect, CaSystem, SoundCoding, State};
use crate::typed::{MacFields, MacStateEvent, EYE_SERIES, MAC_STATE};
use crate::util::cubic;
use sdrkit_algo_dsp::iir::Deemphasis;
use sdrkit_algo_dsp::resample::FractionalResampler;
use sdrkit_core::interface::{Fields as _, Hertz};
use sdrkit_core::publish::keys::Family;
use sdrkit_core::publish::Published;
use sdrkit_core::{Choice, ConfigError, Results, ResultsDecl, SeriesId, Status, Values};
use sdrkit_graph::{
    no_values, Codec, Component, ComponentInfo, Format, Io, Item, Kind, Picture, PixelFormat,
    PixelLayout, Port, RateFrom, Timed,
};
use sdrkit_util_bits::lfsr::Lfsr;
use std::collections::VecDeque;
use std::f64::consts::TAU;
use std::sync::Arc;

/// The sound sampling rate of the MAC sound (ETS 300 250).
pub const SOUND_HZ: f64 = 32_000.0;

/// The display's report to the front: the picture is negative (the spectrum is inverted).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct MacControl {
    /// Invert the discriminator.
    pub inverted: bool,
}
impl Item for MacControl {
    const KIND: &'static str = "mac.control";
}

/// One decoded line.
#[derive(Clone, Debug)]
pub struct MacLine {
    /// Line number (1..625).
    pub number: u32,
    /// Frame count.
    pub frame: u64,
    /// The frame is odd (the frame sync sequence inverted).
    pub odd_frame: bool,
    /// The line sync word was found.
    pub sync_ok: bool,
    /// The variant.
    pub variant: Variant,
    /// Burst decisions (D2: 105, D: 206), or the 648 decisions of line 625.
    pub bits: [u8; 648],
    /// Vision samples after the clamp (volts, 0.5 = the clamp level).
    pub samples: [f32; SAMPLES],
}
impl Item for MacLine {
    const KIND: &'static str = "mac.line";
}

/// The line decoder's report.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LineReport {
    /// 0 searching, 1 line sync, 2 frame sync.
    pub state: u8,
    /// The variant found.
    pub variant: Option<Variant>,
    /// Data SNR, dB.
    pub data_snr_db: f64,
    /// Extreme level, V.
    pub data_level: f64,
    /// Line rate, Hz.
    pub line_hz: f64,
    /// Line error, ppm.
    pub line_ppm: f64,
    /// Share of lines with the sync word, this report.
    pub sync_quality: f64,
    /// Lines.
    pub lines: u64,
    /// Frames.
    pub frames: u64,
    /// Frame syncs found.
    pub frame_syncs: u64,
    /// Bit errors of the frame sync sequence.
    pub bit_errors: u64,
    /// Bits of the frame sync sequence.
    pub bits: u64,
    /// Mean clamp level offset, V.
    pub clamp_offset: f64,
    /// Input level, dBFS.
    pub level_dbfs: f64,
}
impl Item for LineReport {
    const KIND: &'static str = "mac.line_report";
}

/// The discriminator: volts (13.5 MHz/V) plus 0.5, de-emphasised.
pub struct MacDemod {
    fs: f64,
    disc: sdrkit_algo_dsp::fm::Discriminator,
    de: MacEmphasis,
    inverted: bool,
    level: f64,
}

impl Default for MacDemod {
    fn default() -> Self {
        MacDemod {
            fs: 1.0,
            disc: crate::util::phase_steps(),
            de: MacEmphasis::new(20e6, true),
            inverted: false,
            level: -100.0,
        }
    }
}

impl Component for MacDemod {
    fn values(&self) -> sdrkit_core::Values {
        sdrkit_graph::no_values()
    }
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "mac.demod",
            vec![Port::iq("in"), Port::typed::<MacControl>("control")],
            vec![Port::real("out"), Port::typed::<MacLevel>("level")],
        )
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.fs = inputs[0].rate;
        self.de = MacEmphasis::new(self.fs, true);
        Ok(vec![
            inputs[0].with_kind(Kind::Real),
            Format::new(Kind::typed::<MacLevel>(), 0.0),
        ])
    }
    fn process(&mut self, io: &mut Io) {
        if let Some(c) = crate::atv::last::<MacControl>(io, 1) {
            self.inverted = c.inverted;
        }
        let x = io.iq(0);
        let k = self.fs / TAU / HZ_PER_V;
        let sign = if self.inverted { -1.0 } else { 1.0 };
        let mut p = 0.0f64;
        let out = io.real_out(0);
        for &s in x {
            p += s.norm_sqr() as f64;
            let v = self.disc.process_sample(s) as f64 * k * sign;
            out.push((self.de.process(v) + 0.5) as f32);
        }
        if !x.is_empty() {
            self.level += 0.05 * (10.0 * (p / x.len() as f64).max(1e-20).log10() - self.level);
        }
        if io.tick() {
            let t = io.input_format(0).time_of(io.input(0).first());
            io.items_out::<MacLevel>(1).push(Timed {
                time: t,
                value: MacLevel(self.level),
            });
        }
    }
}

/// The input level, dBFS.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct MacLevel(pub f64);
impl Item for MacLevel {
    const KIND: &'static str = "mac.level";
}

/// The line decoder.
pub struct MacLines {
    fs: f64,
    res: Option<FractionalResampler<f32>>,
    corr: f64,
    x: Vec<f32>,
    x0: u64,
    acq: bool,
    variant: Variant,
    line_start: f64,
    number: u32,
    frame: u64,
    odd_frame: bool,
    bad_run: u32,
    since_fs: u32,
    thr: f64,
    extreme: f64,
    err2: f64,
    sync_ok_n: u32,
    lines_n: u32,
    lines: u64,
    frames: u64,
    frame_syncs: u64,
    bit_errors: u64,
    fss_bits: u64,
    clamp_off: f64,
    clamp_sum: f64,
    clamp_n: usize,
    level: f64,
    tmp: Vec<f32>,
}

impl Default for MacLines {
    fn default() -> Self {
        MacLines {
            fs: 1.0,
            res: None,
            corr: 1.0,
            x: Vec::new(),
            x0: 0,
            acq: true,
            variant: Variant::D2,
            line_start: 0.0,
            number: 0,
            frame: 0,
            odd_frame: false,
            bad_run: 0,
            since_fs: 0,
            thr: 0.2,
            extreme: 0.4,
            err2: 0.01,
            sync_ok_n: 0,
            lines_n: 0,
            lines: 0,
            frames: 0,
            frame_syncs: 0,
            bit_errors: 0,
            fss_bits: 0,
            clamp_off: 0.0,
            clamp_sum: 0.0,
            clamp_n: 0,
            level: -100.0,
            tmp: Vec::new(),
        }
    }
}

const ACQ_LINES: usize = 48;

impl MacLines {
    fn at(&self, pos: f64) -> f32 {
        let p = pos - self.x0 as f64;
        cubic(&self.x, p)
    }

    fn word_score(&self, start: f64, v: Variant, centre: f32) -> f64 {
        let mut s = 0.0;
        for (i, &b) in W1.iter().enumerate() {
            let m = (self.at(start + v.bit_pos(i)) - centre).abs() as f64;
            s += if b == 1 { m } else { -m };
        }
        s
    }

    /// Acquisition: the sync word correlation averaged over 48 lines at every position of the line, for both burst
    /// formats; the two halves of the window give the clock error.
    fn acquire(&mut self) -> bool {
        let n = ACQ_LINES * SAMPLES;
        if self.x.len() < n + SAMPLES + 8 {
            return false;
        }
        let centre = crate::util::mean(&self.x[..n]);
        let base = self.x0 as f64;
        let mut best = (0.0f64, 0usize, Variant::D2);
        let mut halves = [[0.0f64; SAMPLES]; 2];
        let mut mean_tot = [0.0f64; 2];
        for (vi, v) in [Variant::D2, Variant::D].into_iter().enumerate() {
            for o in 0..SAMPLES {
                let mut h = [0.0f64; 2];
                for l in 0..ACQ_LINES {
                    let sgn = if l % 2 == 0 { 1.0 } else { -1.0 };
                    h[l / (ACQ_LINES / 2)] +=
                        sgn * self.word_score(base + (l * SAMPLES + o) as f64, v, centre);
                }
                let tot = (h[0] + h[1]).abs();
                mean_tot[vi] += tot / SAMPLES as f64;
                if tot > best.0 {
                    best = (tot, o, v);
                    halves[0][o] = h[0].abs();
                    halves[1][o] = h[1].abs();
                }
                if best.2 == v {
                    halves[0][o] = h[0].abs();
                    halves[1][o] = h[1].abs();
                }
            }
        }
        let (score, o, v) = best;
        // A real burst: about 3 ones of 0.4 V a line, standing far above the correlation at the other places of the line (noise
        // alone produces a peak only a few times its mean).
        let spread = mean_tot[if v == Variant::D2 { 0 } else { 1 }];
        if score < 0.3 * ACQ_LINES as f64 || score < 8.0 * spread {
            self.x.drain(..n);
            self.x0 += n as u64;
            return false;
        }
        self.variant = v;
        self.extreme = 0.4;
        self.thr = 0.2;
        self.line_start = base + o as f64;
        self.acq = false;
        self.bad_run = 0;
        self.number = 0;
        self.since_fs = 0;
        true
    }

    fn decode_line(&mut self, io: &mut Io) {
        let ls = self.line_start;
        let mut cl = 0.0f32;
        for g in CLAMP.0..CLAMP.1 {
            cl += self.at(ls + g as f64);
        }
        let clamp = cl / (CLAMP.1 - CLAMP.0) as f32;
        let next_number = if self.number == 0 {
            0
        } else {
            self.number % LINES + 1
        };
        let mut line = MacLine {
            number: next_number,
            frame: self.frame,
            odd_frame: self.odd_frame,
            sync_ok: false,
            variant: self.variant,
            bits: [0; 648],
            samples: [0.0; SAMPLES],
        };
        // Line 625? Its frame sync sequence at the D2 rate.
        let mut fss_err = [0u32; 2];
        for i in 0..FSS_BITS {
            let v = (self.at(ls + Variant::D2.bit_pos(i)) - clamp).abs() as f64;
            let b = (v > self.thr) as u8;
            if b != fss_bit(i, false) {
                fss_err[0] += 1;
            }
            if b != fss_bit(i, true) {
                fss_err[1] += 1;
            }
        }
        let is625 = fss_err[0].min(fss_err[1]) <= 10
            && (self.number == 0 || next_number == 625 || self.since_fs > 4 * LINES);
        if is625 {
            line.number = 625;
            let odd = fss_err[1] < fss_err[0];
            self.odd_frame = odd;
            line.odd_frame = odd;
            self.frame_syncs += 1;
            self.bit_errors += fss_err[0].min(fss_err[1]) as u64;
            self.fss_bits += FSS_BITS as u64;
            self.since_fs = 0;
            for i in 0..642 {
                let v = (self.at(ls + Variant::D2.bit_pos(i)) - clamp).abs() as f64;
                line.bits[i] = (v > self.thr) as u8;
            }
            line.sync_ok = true;
        } else {
            let nb = self.variant.burst_bits();
            let mut ones = 0.0;
            let mut n1 = 0;
            let mut e2 = 0.0;
            let mut terr = 0.0;
            let mut tn = 0;
            let mut prev: Option<(u8, f32)> = None;
            for i in 0..nb {
                let v = self.at(ls + self.variant.bit_pos(i)) - clamp;
                let b = (v.abs() as f64 > self.thr) as u8;
                line.bits[i] = b;
                if b == 1 {
                    ones += v.abs() as f64;
                    n1 += 1;
                    e2 += (v.abs() as f64 - self.extreme).powi(2);
                } else {
                    e2 += (v as f64).powi(2);
                }
                // Timing: a zero between ones of opposite polarity is crossed at its centre.
                if let (Some((1, pv)), true) = (prev, b == 0) {
                    if i + 1 < nb {
                        let nv = self.at(ls + self.variant.bit_pos(i + 1)) - clamp;
                        if (nv.abs() as f64) > self.thr && nv.signum() != pv.signum() {
                            terr += (v * pv.signum()) as f64;
                            tn += 1;
                        }
                    }
                }
                prev = Some((b, v));
            }
            if n1 > 0 {
                self.extreme += 0.02 * (ones / n1 as f64 - self.extreme);
                self.thr = 0.5 * self.extreme;
            }
            self.err2 += 0.02 * (e2 / nb as f64 - self.err2);
            let w = sync_word(1);
            let errs_w1 = (0..6).filter(|&i| line.bits[i] != w[i]).count();
            let errs_w2 = (0..6).filter(|&i| line.bits[i] == w[i]).count();
            line.sync_ok = errs_w1.min(errs_w2) <= 1;
            if tn > 0 {
                // The slope of a crossing is about two extreme levels per two bit periods.
                let per = match self.variant {
                    Variant::D2 => 2.0,
                    Variant::D => 1.0,
                };
                let slope = self.extreme / per;
                let e = (terr / tn as f64) / slope.max(1e-6);
                self.line_start += 0.1 * e.clamp(-1.0, 1.0);
                self.corr += 2e-6 * e.clamp(-1.0, 1.0);
                self.corr = self.corr.clamp(0.999, 1.001);
                if let Some(r) = &mut self.res {
                    r.set_correction(self.corr);
                }
            }
        }
        if line.sync_ok {
            self.bad_run = 0;
            self.sync_ok_n += 1;
        } else {
            self.bad_run += 1;
        }
        self.lines_n += 1;
        self.lines += 1;
        self.since_fs += 1;
        if line.number == 625 {
            self.frames += 1;
            self.frame = self.frames;
        }
        self.number = line.number;
        if self.since_fs > 4 * LINES && self.number != 0 {
            self.number = 0;
        }
        self.clamp_sum += (clamp - 0.5) as f64;
        self.clamp_n += 1;
        if self.clamp_n >= LINES as usize {
            self.clamp_off = self.clamp_sum / self.clamp_n as f64;
            self.clamp_sum = 0.0;
            self.clamp_n = 0;
        }
        for g in 0..SAMPLES {
            line.samples[g] = self.at(ls + g as f64) - clamp + 0.5;
        }
        let t = ls / CLOCK;
        io.items_out::<MacLine>(0).push(Timed {
            time: t,
            value: line,
        });
        self.line_start += SAMPLES as f64;
        if self.bad_run > 60 {
            self.acq = true;
        }
    }
}

impl Component for MacLines {
    fn values(&self) -> sdrkit_core::Values {
        sdrkit_graph::no_values()
    }
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "mac.lines",
            vec![Port::real("in"), Port::typed::<MacLevel>("level")],
            vec![
                Port::typed::<MacLine>("lines"),
                Port::typed::<LineReport>("report"),
            ],
        )
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.fs = inputs[0].rate;
        let pass = (0.45 * self.fs.min(CLOCK)).min(9.5e6);
        self.res = FractionalResampler::new(
            self.fs,
            CLOCK,
            pass,
            (self.fs.min(CLOCK) - pass).max(pass + 0.2e6),
            50.0,
        )
        .or_else(|_| FractionalResampler::with_rates(self.fs, CLOCK))
        .ok();
        *self = MacLines {
            fs: self.fs,
            res: self.res.take(),
            ..Default::default()
        };
        Ok(vec![
            Format::new(Kind::typed::<MacLine>(), 0.0),
            Format::new(Kind::typed::<LineReport>(), 0.0),
        ])
    }
    fn process(&mut self, io: &mut Io) {
        if let Some(l) = crate::atv::last::<MacLevel>(io, 1) {
            self.level = l.0;
        }
        let input = io.real(0);
        let mut tmp = std::mem::take(&mut self.tmp);
        tmp.clear();
        if let Some(r) = &mut self.res {
            r.process(input, &mut tmp);
        }
        self.x.extend_from_slice(&tmp);
        self.tmp = tmp;
        loop {
            if self.acq {
                if !self.acquire() {
                    break;
                }
                continue;
            }
            let need = self.line_start - self.x0 as f64 + SAMPLES as f64 + 8.0;
            if (self.x.len() as f64) < need {
                break;
            }
            self.decode_line(io);
            if self.acq {
                continue;
            }
        }
        // Drop what is decoded.
        if !self.acq {
            let keep_from = (self.line_start - self.x0 as f64 - 8.0).floor().max(0.0) as usize;
            let k = keep_from.min(self.x.len());
            self.x.drain(..k);
            self.x0 += k as u64;
        }
        if io.tick() {
            let state = if self.acq {
                0
            } else if self.since_fs <= 2 * LINES && self.frame_syncs > 0 {
                2
            } else {
                1
            };
            let lh = CLOCK / self.corr / SAMPLES as f64;
            let rep = LineReport {
                state,
                variant: (!self.acq).then_some(self.variant),
                data_snr_db: (20.0 * (self.extreme / self.err2.max(1e-12).sqrt()).log10())
                    .clamp(0.0, 60.0),
                data_level: self.extreme,
                line_hz: lh * self.fs / self.fs,
                line_ppm: (1.0 / self.corr - 1.0) * 1e6,
                sync_quality: if self.lines_n > 0 {
                    self.sync_ok_n as f64 / self.lines_n as f64
                } else {
                    0.0
                },
                lines: self.lines,
                frames: self.frames,
                frame_syncs: self.frame_syncs,
                bit_errors: self.bit_errors,
                bits: self.fss_bits,
                clamp_offset: self.clamp_off,
                level_dbfs: self.level,
            };
            self.sync_ok_n = 0;
            self.lines_n = 0;
            let t = io.input_format(0).time_of(io.input(0).first());
            io.items_out::<LineReport>(1).push(Timed {
                time: t,
                value: rep,
            });
        }
    }
}

/// The demultiplexer's report.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MuxReport {
    /// Packet headers decoded.
    pub headers: u64,
    /// Headers with corrections.
    pub corrected: u64,
    /// Dummy packets.
    pub dummy: u64,
    /// SI groups good.
    pub si_good: u64,
    /// SI groups bad.
    pub si_bad: u64,
    /// SI seen.
    pub si_seen: bool,
    /// Channel name.
    pub channel: String,
    /// Network name.
    pub network: String,
    /// Country.
    pub country: String,
    /// Satellite channel.
    pub sat_channel: u32,
    /// Sound address.
    pub sound_address: Option<u16>,
    /// Sound scrambled.
    pub sound_scrambled: bool,
    /// Sound packets.
    pub sound_packets: u64,
    /// Sound errors (continuity and parity).
    pub sound_errors: u64,
    /// Sound level, dBFS.
    pub sound_level_dbfs: f64,
    /// Sound playing.
    pub sound_playing: bool,
    /// Conditional access seen.
    pub ca: bool,
    /// The conditional access system named.
    pub ca_system: Option<CaSystem>,
    /// ECM address.
    pub ecm_address: Option<u16>,
    /// ECM packets.
    pub ecm_packets: u64,
    /// Wide (from VCONF), when known.
    pub vconf_wide: Option<bool>,
    /// The SDF.
    pub sdf: Option<Sdf>,
    /// The frame count of the RDF.
    pub fcnt: Option<u8>,
    /// Line 625 codewords that failed.
    pub bad_blocks: u64,
    /// Log lines.
    pub log: Vec<String>,
}
impl Item for MuxReport {
    const KIND: &'static str = "mac.mux_report";
}

/// The packet demultiplexer, service identification and sound.
pub struct MacMux {
    sub: [Vec<u8>; 2],
    prbs: Lfsr,
    variant: Variant,
    last_line: u32,
    rep: MuxReport,
    si_buf: Vec<u8>,
    si_ci: Option<u8>,
    si_need: usize,
    snd_ci: Option<u8>,
    cand: std::collections::HashMap<u16, u32>,
    prev: i32,
    deemph: Deemphasis,
    out32: Vec<f32>,
    frames_since_tv: u32,
    tv_seen: bool,
    level_acc: f64,
    level_n: usize,
    sdf_last: Option<u8>,
}

impl Default for MacMux {
    fn default() -> Self {
        MacMux {
            sub: [Vec::new(), Vec::new()],
            prbs: dispersal(),
            variant: Variant::D2,
            last_line: 0,
            rep: MuxReport::default(),
            si_buf: Vec::new(),
            si_ci: None,
            si_need: 0,
            snd_ci: None,
            cand: Default::default(),
            prev: 0,
            deemph: crate::nicam::j17_de(),
            out32: Vec::new(),
            frames_since_tv: 0,
            tv_seen: false,
            level_acc: 0.0,
            level_n: 0,
            sdf_last: None,
        }
    }
}

impl MacMux {
    fn params(&mut self, ci: u8, params: &[super::packet::Param], in_ca: bool) {
        for p in params {
            match (ci, p.pi) {
                (ids::CI_NETWORK, ids::NWO) => {
                    if !p.data.is_empty() {
                        let b = p.data[0];
                        self.rep.sat_channel = ((b >> 4) * 10 + (b & 15)) as u32;
                    }
                    if p.data.len() > 3 {
                        self.rep.country = text_of(&p.data[3..]);
                    }
                }
                (ids::CI_NETWORK, ids::NWNAME) => {
                    self.rep.network = text_of(&p.data);
                    if self.rep.channel.is_empty() {
                        self.rep.channel = self.rep.network.clone();
                    }
                }
                (ids::CI_NETWORK, ids::LISTX) => {
                    if p.data.len() >= 3 && p.data[0] == 1 && self.rep.sound_address.is_none() {
                        self.rep.sound_address =
                            Some(p.data[1] as u16 | ((p.data[2] as u16 & 3) << 8));
                    }
                }
                (ids::CI_TV, ids::SREF) => {
                    if p.data.len() > 1 {
                        let name = text_of(&p.data[1..]);
                        if name != self.rep.channel || !self.tv_seen {
                            self.rep.log.push(format!("channel \"{name}\""));
                        }
                        self.rep.channel = name;
                    }
                }
                (ids::CI_TV, ids::VCONF) => {
                    if let Some(&b) = p.data.first() {
                        self.rep.vconf_wide = Some((b >> 2) & 7 == 6);
                    }
                }
                (ids::CI_TV, ids::ACCM) => {
                    if p.data.len() >= 3 {
                        self.rep.ca = true;
                        let a = p.data[0] as u16 | ((p.data[1] as u16) << 8);
                        if a & 0x8000 != 0 {
                            self.rep.ecm_address = Some(a & 0x3FF);
                        }
                        self.rep.ca_system = Some(if p.data[2] == ids::EUROCRYPT {
                            CaSystem::EuroCrypt
                        } else {
                            CaSystem::Other
                        });
                    }
                }
                (ids::CI_TV, ids::TV_SOUND) => {
                    if p.data.len() >= 2 {
                        self.rep.sound_address =
                            Some(p.data[0] as u16 | ((p.data[1] as u16 & 3) << 8));
                        self.rep.sound_scrambled = in_ca;
                        self.tv_seen = true;
                        self.frames_since_tv = 0;
                    }
                }
                (ids::CI_TV, ids::PG) => {
                    let inner = decode_params(&p.data);
                    let ca = inner.iter().any(|q| q.pi == ids::ACCM);
                    self.params(ci, &inner, ca);
                }
                _ => {}
            }
        }
    }

    fn packet(&mut self, pk: Packet, corr: u32, io: &mut Io) {
        self.rep.headers += 1;
        if corr > 0 {
            self.rep.corrected += 1;
        }
        if pk.address == DUMMY {
            self.rep.dummy += 1;
            return;
        }
        if pk.address == SI_ADDRESS {
            self.rep.si_seen = true;
            match pk.data[0] {
                SI_FIRST => {
                    self.si_buf.clear();
                    self.si_buf.extend_from_slice(&pk.data[1..89]);
                    self.si_ci = Some(pk.ci);
                    self.si_need = group_len(&self.si_buf).unwrap_or(0);
                }
                NEXT => match self.si_ci {
                    Some(c) if (c + 1) & 3 == pk.ci => {
                        self.si_buf.extend_from_slice(&pk.data[1..89]);
                        self.si_ci = Some(pk.ci);
                    }
                    Some(_) => {
                        self.rep.si_bad += 1;
                        self.si_ci = None;
                        self.si_need = 0;
                    }
                    None => {}
                },
                _ => {}
            }
            if self.si_need > 0 && self.si_buf.len() >= self.si_need {
                match decode_group(&self.si_buf) {
                    Some((_, cmds)) => {
                        self.rep.si_good += 1;
                        for c in &cmds {
                            self.params(c.ci, &c.params, false);
                        }
                    }
                    None => self.rep.si_bad += 1,
                }
                self.si_need = 0;
                self.si_ci = None;
            }
            return;
        }
        if Some(pk.address) == self.rep.ecm_address {
            self.rep.ecm_packets += 1;
            return;
        }
        if pk.data[0] == NEXT && self.rep.sound_address.is_none() {
            let c = self.cand.entry(pk.address).or_insert(0);
            *c += 1;
            if *c >= 8 {
                self.rep.sound_address = Some(pk.address);
                self.rep
                    .log
                    .push(format!("sound packets on address {}", pk.address));
            }
        }
        if Some(pk.address) == self.rep.sound_address && pk.data[0] == NEXT {
            self.rep.sound_packets += 1;
            if let Some(c) = self.snd_ci {
                if (c + 1) & 3 != pk.ci {
                    self.rep.sound_errors += 1;
                }
            }
            self.snd_ci = Some(pk.ci);
            let clear = !self.rep.sound_scrambled && (self.tv_seen || self.frames_since_tv > 50);
            self.rep.sound_playing = clear;
            if !clear {
                return;
            }
            let mut x = [0i32; 64];
            let bad = decode_sound(&pk.data[1..91], &mut self.prev, &mut x);
            self.rep.sound_errors += bad as u64;
            for &v in &x {
                let y = self.deemph.process_f64(v as f64 / 8192.0) as f32;
                self.level_acc += (y as f64).powi(2);
                self.level_n += 1;
                self.out32.push(y);
            }
            if self.level_n >= 3200 {
                self.rep.sound_level_dbfs =
                    10.0 * (self.level_acc / self.level_n as f64).max(1e-12).log10();
                self.level_acc = 0.0;
                self.level_n = 0;
            }
            let _ = io;
        }
    }

    fn end_frame(&mut self, io: &mut Io) {
        for s in 0..self.variant.subframes() {
            if self.sub[s].len() >= PACKETS * PACKET_BITS {
                for k in 0..PACKETS {
                    let (pk, corr) = Packet::from_bits(
                        &self.sub[s][k * PACKET_BITS..(k + 1) * PACKET_BITS],
                        header_code(),
                    );
                    if s == 0 {
                        self.packet(pk, corr, io);
                    } else {
                        self.rep.headers += 1;
                        if pk.address == DUMMY {
                            self.rep.dummy += 1;
                        }
                    }
                }
            }
            self.sub[s].clear();
        }
        self.frames_since_tv += 1;
    }
}

impl Component for MacMux {
    fn values(&self) -> sdrkit_core::Values {
        sdrkit_graph::no_values()
    }
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "mac.mux",
            vec![Port::typed::<MacLine>("lines")],
            vec![
                Port::audio("out").rate_from(RateFrom::Fixed(SOUND_HZ)),
                Port::typed::<MuxReport>("report").rate_from(RateFrom::Fixed(0.0)),
            ],
        )
    }
    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        *self = MacMux::default();
        Ok(vec![
            Format::audio(SOUND_HZ, 1),
            Format::new(Kind::typed::<MuxReport>(), 0.0),
        ])
    }
    fn process(&mut self, io: &mut Io) {
        self.out32.clear();
        let lines = io.items::<MacLine>(0);
        for l in lines {
            let line = &l.value;
            self.variant = line.variant;
            if line.number == 0 {
                continue;
            }
            if line.number == 1 || (line.number < self.last_line && line.number != 625) {
                self.sub[0].clear();
                self.sub[1].clear();
                self.prbs = dispersal();
            }
            if (1..=623).contains(&line.number) {
                let nb = line.variant.burst_bits();
                let mut prb = Vec::with_capacity(nb);
                for _ in 0..nb - 6 {
                    prb.push(self.prbs.bit());
                }
                for _ in nb..line.variant.periods() {
                    self.prbs.bit();
                }
                for i in 0..99 {
                    self.sub[0].push(line.bits[6 + i] ^ prb[i]);
                }
                if line.variant == Variant::D {
                    for i in 0..99 {
                        self.sub[1].push(line.bits[105 + i] ^ prb[99 + i]);
                    }
                }
                if line.number == 623 {
                    self.end_frame(io);
                }
            }
            if line.number == 625 {
                let (sdf, rdf, bad) = decode_special(&line.bits[FSS_BITS..FSS_BITS + 546]);
                self.rep.bad_blocks += bad as u64;
                if let Some(s) = sdf {
                    if self.sdf_last != Some(s.mvscg) {
                        self.sdf_last = Some(s.mvscg);
                        self.rep.log.push(format!(
                            "line 625: channel id {:04X}, {}, {}",
                            s.chid,
                            if s.narrow() { "4:3" } else { "16:9" },
                            s.access_text()
                        ));
                    }
                    self.rep.sdf = Some(s);
                }
                if let Some(r) = rdf {
                    self.rep.fcnt = Some(r.fcnt);
                }
            }
            self.last_line = line.number;
        }
        io.audio_out(0).samples.extend_from_slice(&self.out32);
        if io.tick() {
            let mut r = self.rep.clone();
            r.log = std::mem::take(&mut self.rep.log);
            io.items_out::<MuxReport>(1).push(Timed {
                time: 0.0,
                value: r,
            });
        }
    }
}

/// The picture builder's report.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct MacPictureReport {
    /// Pictures.
    pub pictures: u64,
    /// Black level, V.
    pub black: f64,
    /// White level, V.
    pub white: f64,
    /// Video SNR, dB.
    pub video_snr_db: f64,
    /// The picture was negative (inverted spectrum).
    pub negative: bool,
    /// Width in use.
    pub width: u32,
}
impl Item for MacPictureReport {
    const KIND: &'static str = "mac.picture_report";
}

/// The picture builder.
pub struct MacPicture {
    luma: Vec<f32>,
    chroma: Vec<f32>,
    have: Vec<bool>,
    black: f64,
    white: f64,
    refs: bool,
    noise: f64,
    rep: MacPictureReport,
    /// 16:9 (from the SDF aspect bit or VCONF).
    pub wide: bool,
}

impl Default for MacPicture {
    fn default() -> Self {
        let nl = LUMA.1 - LUMA.0;
        let nc = COLOUR.1 - COLOUR.0;
        MacPicture {
            luma: vec![0.0; 626 * nl],
            chroma: vec![0.5; 626 * nc],
            have: vec![false; 626],
            black: 0.0,
            white: 1.0,
            refs: false,
            noise: 1e-3,
            rep: MacPictureReport::default(),
            wide: false,
        }
    }
}

impl MacPicture {
    fn build(&mut self, io: &mut Io, t: f64) {
        let got = (23..=622).filter(|&l| self.have[l]).count();
        if got < 400 {
            return;
        }
        let w = if self.wide { 1024 } else { 768 };
        let h = 576;
        let nl = LUMA.1 - LUMA.0;
        let nc = COLOUR.1 - COLOUR.0;
        let range = (self.white - self.black).max(0.3) as f32;
        let mut px = vec![255u8; w * h * 4];
        for row in 0..h {
            let l = if row % 2 == 0 {
                23 + row / 2
            } else {
                335 + row / 2
            };
            if !self.have[l] {
                continue;
            }
            let lc = l - 1;
            let comp = |line: usize, x: f64, s: &Self| -> f32 {
                if line < 1 || !s.have[line] {
                    return 0.5;
                }
                let k = (x * nc as f64 - 0.5 + 4.0 / 3.0).clamp(0.0, (nc - 1) as f64);
                let i0 = k.floor() as usize;
                let f = (k - i0 as f64) as f32;
                let a = s.chroma[line * nc + i0];
                let b = s.chroma[line * nc + (i0 + 1).min(nc - 1)];
                a + (b - a) * f
            };
            for x in 0..w {
                let fx = (x as f64 + 0.5) / w as f64;
                let k = (fx * nl as f64 - 0.5).clamp(0.0, (nl - 1) as f64);
                let i0 = k.floor() as usize;
                let f = (k - i0 as f64) as f32;
                let a = self.luma[l * nl + i0];
                let b = self.luma[l * nl + (i0 + 1).min(nl - 1)];
                let y = ((a + (b - a) * f) - self.black as f32) / range;
                // The line's own component from the line before; the other from the lines two apart.
                let own = comp(lc, fx, self) - 0.5;
                let other = 0.5
                    * ((comp(lc.saturating_sub(1), fx, self) - 0.5) + (comp(l, fx, self) - 0.5));
                let (cu, cv) = if lc % 2 == 1 {
                    (own, other)
                } else {
                    (other, own)
                };
                let by = cu / range / MAC_KU as f32;
                let ry = cv / range / MAC_KV as f32;
                let rgb = sdrkit_codec_image::colour::rgb_from_differences(
                    sdrkit_codec_image::colour::BT601,
                    y,
                    by,
                    ry,
                );
                let i = (row * w + x) * 4;
                px[i] = (rgb[2].clamp(0.0, 1.0) * 255.0).round() as u8;
                px[i + 1] = (rgb[1].clamp(0.0, 1.0) * 255.0).round() as u8;
                px[i + 2] = (rgb[0].clamp(0.0, 1.0) * 255.0).round() as u8;
            }
        }
        let pic = Picture {
            codec: Codec::Pixels(PixelFormat::Bgra),
            layout: Some(PixelLayout::packed(PixelFormat::Bgra, w as u32, h as u32)),
            name: None,
            data: Arc::from(px.into_boxed_slice()),
        };
        self.rep.pictures += 1;
        self.rep.width = w as u32;
        io.items_out::<Picture>(0).push(Timed {
            time: t,
            value: pic,
        });
        for v in self.have.iter_mut() {
            *v = false;
        }
    }
}

impl Component for MacPicture {
    fn values(&self) -> sdrkit_core::Values {
        sdrkit_graph::no_values()
    }
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "mac.picture",
            vec![
                Port::typed::<MacLine>("lines"),
                Port::typed::<MuxReport>("mux"),
            ],
            vec![
                Port::typed::<Picture>("out").rate_from(RateFrom::Fixed(0.0)),
                Port::typed::<MacPictureReport>("report").rate_from(RateFrom::Fixed(0.0)),
            ],
        )
    }
    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        *self = MacPicture::default();
        Ok(vec![
            Format::new(Kind::typed::<Picture>(), 0.0),
            Format::new(Kind::typed::<MacPictureReport>(), 0.0),
        ])
    }
    fn process(&mut self, io: &mut Io) {
        if let Some(m) = crate::atv::last::<MuxReport>(io, 1) {
            if let Some(s) = m.sdf {
                self.wide = !s.narrow();
            } else if let Some(w) = m.vconf_wide {
                self.wide = w;
            }
        }
        let nl = LUMA.1 - LUMA.0;
        let nc = COLOUR.1 - COLOUR.0;
        for item in io.items::<MacLine>(0) {
            let l = &item.value;
            let n = l.number as usize;
            if n == 0 {
                continue;
            }
            if (23..=622).contains(&n) {
                self.luma[n * nl..(n + 1) * nl].copy_from_slice(&l.samples[LUMA.0..LUMA.1]);
                self.chroma[n * nc..(n + 1) * nc].copy_from_slice(&l.samples[COLOUR.0..COLOUR.1]);
                self.have[n] = true;
            }
            if n == 624 {
                let m = |a: usize, b: usize| crate::util::mean(&l.samples[a + 8..b - 8]) as f64;
                let (wv, bv) = (m(REF_WHITE.0, REF_WHITE.1), m(REF_BLACK.0, REF_BLACK.1));
                if wv - bv < -0.3 {
                    if !self.rep.negative {
                        self.rep.negative = true;
                    }
                } else if wv - bv >= 0.3 {
                    if self.refs {
                        self.black += 0.2 * (bv - self.black);
                        self.white += 0.2 * (wv - self.white);
                    } else {
                        self.black = bv;
                        self.white = wv;
                        self.refs = true;
                    }
                    let s = &l.samples[REF_WHITE.0 + 8..REF_WHITE.1 - 8];
                    let mean = crate::util::mean(s);
                    let var = s.iter().map(|v| (v - mean).powi(2)).sum::<f32>() / s.len() as f32;
                    self.noise += 0.2 * (var as f64 - self.noise);
                }
            }
            if n == 625 {
                self.build(io, item.time);
            }
        }
        if io.tick() {
            self.rep.black = self.black;
            self.rep.white = self.white;
            self.rep.video_snr_db = (20.0
                * ((self.white - self.black).max(1e-6) / self.noise.max(1e-12).sqrt()).log10())
            .clamp(0.0, 60.0);
            io.items_out::<MacPictureReport>(1).push(Timed {
                time: 0.0,
                value: self.rep,
            });
        }
    }
}

/// The status snapshot of the `mac` stack at each report. The results fields are written from it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MacDetail {
    /// 0 searching, 1 line sync, 2 frame sync.
    pub state: u8,
    /// The variant, once line sync holds.
    pub variant: Option<Variant>,
    /// The line decoder's measurements.
    pub lines: LineReport,
    /// The demultiplexer.
    pub mux: MuxReport,
    /// The picture.
    pub picture: MacPictureReport,
    /// Carrier offset, Hz.
    pub carrier_offset_hz: f64,
    /// Deviation, MHz/V.
    pub deviation_mhz_per_v: f64,
    /// The aspect ratio, once known.
    pub aspect: Option<Aspect>,
    /// The access conditions, in words.
    pub scrambling: String,
    /// Scrambled.
    pub scrambled: bool,
    /// Sound present.
    pub sound_present: bool,
    /// How the sound is coded.
    pub sound_mode: SoundCoding,
    /// Spectrum inverted.
    pub inverted: bool,
    /// Bit error rate.
    pub bit_error_rate: f64,
    /// Summary.
    pub summary: String,
}

/// The writer of the MAC display: the result fields and the eye series.
fn mac_published() -> Published {
    Published::from_decl(Arc::new(
        ResultsDecl::new()
            .with_fields(MacFields::SHAPE.fields)
            .series(EYE_SERIES, "Eye", "", ""),
    ))
}

/// Writes the snapshot `d` as the result fields.
fn publish_mac(p: &mut Published, d: &MacDetail) {
    let l = &d.lines;
    let m = &d.mux;
    fn text(v: &str) -> Option<&str> {
        (!v.is_empty()).then_some(v)
    }
    p.put(&MacFields {
        state: State::of(d.state),
        system: d.variant,
        snr_db: l.data_snr_db,
        video_snr_db: d.picture.video_snr_db,
        offset_hz: Hertz(d.carrier_offset_hz),
        level_dbfs: l.level_dbfs,
        deviation_hz_per_v: d.deviation_mhz_per_v * 1e6,
        spectrum_inverted: d.inverted,
        line_rate: Hertz(l.line_hz),
        line_error_ppm: l.line_ppm,
        line_sync_pct: 100.0 * l.sync_quality,
        frame_syncs: l.frame_syncs as i64,
        frames: l.frames as i64,
        ber: d.bit_error_rate,
        channel: text(&m.channel),
        network: text(&m.network),
        country: text(&m.country),
        channel_id: m.sdf.map(|s| i64::from(s.chid)),
        aspect: d.aspect,
        scrambled: d.scrambled,
        access: text(&d.scrambling),
        ca_system: m.ca_system,
        sound_errors: m.sound_errors as i64,
        ecm_packets: m.ecm_packets as i64,
        packet_headers: m.headers as i64,
        packets_corrected: m.corrected as i64,
        packets_dummy: m.dummy as i64,
        si_groups_ok: m.si_good as i64,
        si_groups_bad: m.si_bad as i64,
        sound_address: m.sound_address.map(i64::from),
        sound_present: d.sound_present,
        sound_packets: m.sound_packets as i64,
        sound_mode: d.sound_mode,
        audio_level_dbfs: m.sound_level_dbfs,
        blocks_bad: m.bad_blocks as i64,
        pictures: d.picture.pictures as i64,
    });
}

/// The MAC display: results, the status snapshot, and the inversion fed back to the discriminator.
pub struct MacDisplay {
    published: Published,
    eye: Option<SeriesId>,
    detail: MacDetail,
    ticks: u64,
    lines: LineReport,
    mux: MuxReport,
    pic: MacPictureReport,
    inverted: bool,
    was_state: State,
    log: VecDeque<String>,
}

impl Default for MacDisplay {
    fn default() -> Self {
        MacDisplay {
            published: mac_published(),
            eye: None,
            detail: MacDetail::default(),
            ticks: 0,
            lines: LineReport::default(),
            mux: MuxReport::default(),
            pic: MacPictureReport::default(),
            inverted: false,
            was_state: State::Searching,
            log: VecDeque::new(),
        }
    }
}

impl Component for MacDisplay {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "mac.display",
            vec![
                Port::typed::<LineReport>("lines"),
                Port::typed::<MuxReport>("mux"),
                Port::typed::<MacPictureReport>("picture"),
            ],
            vec![
                Port::typed::<MacControl>("control").rate_from(RateFrom::Fixed(0.0)),
                Port::new("events", Kind::Messages).rate_from(RateFrom::Fixed(0.0)),
            ],
        )
    }
    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        *self = MacDisplay::default();
        self.eye = series_id(&self.published, EYE_SERIES);
        Ok(vec![
            Format::new(Kind::typed::<MacControl>(), 0.0),
            Format::new(Kind::Messages, 0.0),
        ])
    }
    fn process(&mut self, io: &mut Io) {
        if let Some(l) = crate::atv::last::<LineReport>(io, 0) {
            self.lines = l;
        }
        for m in io.items::<MuxReport>(1) {
            for l in &m.value.log {
                self.log.push_back(l.clone());
            }
            self.mux.clone_from(&m.value);
        }
        if let Some(p) = crate::atv::last::<MacPictureReport>(io, 2) {
            if p.negative && !self.inverted {
                self.inverted = true;
                self.log
                    .push_back("the picture is negative: spectrum inverted, corrected".into());
            }
            self.pic = p;
        }
        if !io.tick() {
            return;
        }
        self.ticks += 1;
        io.items_out::<MacControl>(0).push(Timed {
            time: 0.0,
            value: MacControl {
                inverted: self.inverted,
            },
        });
        if !self.ticks.is_multiple_of(25) {
            return;
        }
        let d = &mut self.detail;
        d.state = self.lines.state;
        d.variant = if d.state > 0 {
            self.lines.variant
        } else {
            None
        };
        d.lines = self.lines;
        d.mux.clone_from(&self.mux);
        d.mux.log.clear();
        d.picture = self.pic;
        d.inverted = self.inverted;
        let sign = if self.inverted { -1.0 } else { 1.0 };
        // The de-emphasis has a DC gain of 1 / A (A = 1 / sqrt 2): a carrier offset reads 1 / A times larger at the clamp.
        d.carrier_offset_hz =
            sign * self.lines.clamp_offset * HZ_PER_V * std::f64::consts::FRAC_1_SQRT_2;
        d.deviation_mhz_per_v = 13.5 * self.lines.data_level / 0.4;
        d.aspect = match (self.mux.sdf, self.mux.vconf_wide) {
            (Some(s), _) => Some(if s.narrow() {
                Aspect::Normal
            } else {
                Aspect::Wide
            }),
            (None, Some(w)) => Some(if w { Aspect::Wide } else { Aspect::Normal }),
            _ => None,
        };
        d.scrambled = self.mux.sdf.is_some_and(|s| s.scrambled()) || self.mux.ca;
        d.scrambling = self.mux.sdf.map(|s| s.access_text()).unwrap_or_default();
        d.sound_present = d.state == 2 && self.mux.sound_packets > 0 && !self.mux.sound_scrambled;
        d.sound_mode = if self.mux.sound_scrambled {
            SoundCoding::Scrambled
        } else {
            SoundCoding::Companded
        };
        let b = self.lines.bits.max(1) as f64;
        let mut ber = self.lines.bit_errors as f64 / b;
        if self.lines.bit_errors > 96 * 50 {
            ber /= 2.0;
        }
        d.bit_error_rate = ber;
        d.summary = if d.state == 0 {
            "MAC: searching".into()
        } else {
            let access = if !d.scrambled {
                "clear".to_string()
            } else if let Some(ca) = self.mux.ca_system {
                format!("scrambled ({})", ca.label())
            } else {
                "scrambled".to_string()
            };
            format!(
                "MAC: {}  {}  {}  data SNR {:.1} dB  packets {}  {}{}",
                d.variant.map_or("", |v| v.name()),
                self.mux.channel,
                d.aspect.map_or("", |a| a.label()),
                self.lines.data_snr_db,
                self.mux.headers,
                access,
                if d.state == 1 {
                    " (line sync only)"
                } else {
                    ""
                }
            )
        };
        publish_mac(&mut self.published, &self.detail);
        let d = &self.detail;
        let state = State::of(d.state);
        let r = self.published.results_mut();
        r.set_status(&Status {
            lock_state: state.lock_state(),
            snr_db: Some(self.lines.data_snr_db as f32),
            level_dbfs: Some(self.lines.level_dbfs as f32),
            offset_hz: Some(d.carrier_offset_hz),
            summary: d.summary.clone(),
        });
        while let Some(l) = self.log.pop_front() {
            r.log(l);
        }
        if let Some(id) = self.eye {
            r.update_series(id, |s| {
                let changed = s.line;
                s.line = false;
                changed
            });
        }
        if state != self.was_state {
            let t = self.ticks as f64 * 0.01;
            io.messages_out(1).push(MAC_STATE.event(
                t,
                0.0,
                &MacStateEvent {
                    state,
                    text: &d.summary,
                    family: Family::Receiver,
                },
            ));
            self.was_state = state;
        }
    }
    fn values(&self) -> Values {
        no_values()
    }
    fn results(&self) -> Option<&Results> {
        Some(self.published.results())
    }
}
