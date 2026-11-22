//! NBTV and mechanical television: the format, the radio side (AM envelope or narrow FM to an audio-rate video signal), the
//! decoder and the generator.

use super::{last, NbtvReport, Tune};
use crate::card::nbtv_cell;
use crate::util::RingRead;
use sdrkit_algo_dsp::decim::Decimator;
use sdrkit_core::{Complex32, ConfigError, Generator, Hz, Values};
use sdrkit_graph::{
    no_values, Codec, Component, ComponentInfo, Format, Io, Kind, Picture, PixelFormat,
    PixelLayout, Port, RateFrom, Timed,
};
use std::f64::consts::TAU;
use std::sync::Arc;

/// An NBTV or mechanical television format.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NbtvFormat {
    /// Lines a picture (24 to 240).
    pub lines: u32,
    /// Pictures a second.
    pub rate: f64,
    /// Vertical scan (each line runs from top to bottom).
    pub vertical: bool,
}

impl Default for NbtvFormat {
    fn default() -> Self {
        NbtvFormat {
            lines: 32,
            rate: 12.5,
            vertical: true,
        }
    }
}

/// Line sync, black porch after it, black before the next sync (fractions of a line).
pub const SYNC: f64 = 0.06;
/// Black porch after the sync.
pub const PORCH: f64 = 0.03;
/// Black before the next sync.
pub const FRONT: f64 = 0.01;
/// The active fraction of a line.
pub const ACTIVE: f64 = 1.0 - SYNC - PORCH - FRONT;

impl NbtvFormat {
    /// Lines a second.
    pub fn line_hz(&self) -> f64 {
        self.lines as f64 * self.rate
    }
    /// Samples along a line (aspect 2:3, width : height).
    pub fn samples(&self) -> usize {
        let a = 2.0 / 3.0;
        let s = if self.vertical {
            self.lines as f64 / a
        } else {
            self.lines as f64 * a
        };
        (s.round() as usize).max(8)
    }
    /// Video bandwidth, Hz.
    pub fn video_hz(&self) -> f64 {
        0.5 * self.line_hz() * self.samples() as f64 / ACTIVE
    }
    /// The name ("NBTV 32 lines 12.5/s", "mechanical 60 lines 25/s").
    pub fn name(&self) -> String {
        if self.lines == 32 && (self.rate - 12.5).abs() < 1e-9 {
            "NBTV 32 lines 12.5/s".into()
        } else {
            let r = if self.rate.fract() == 0.0 {
                format!("{:.0}", self.rate)
            } else {
                format!("{}", self.rate)
            };
            format!("mechanical {} lines {r}/s", self.lines)
        }
    }
    /// The video level at time `t` (seconds) for picture `pattern` (sync 0, black 0.3, white 1).
    pub fn level(&self, t: f64, pattern: u32) -> f64 {
        let lt = t * self.line_hz();
        let line = (lt.floor() as i64).rem_euclid(self.lines as i64) as usize;
        let x = lt - lt.floor();
        let sync = if line == 0 { 3.0 * SYNC } else { SYNC };
        if x < sync {
            0.0
        } else if x < SYNC + PORCH || x >= 1.0 - FRONT || (line == 0 && x < sync + PORCH) {
            0.3
        } else {
            let s = self.samples();
            let cell = (((x - SYNC - PORCH) / ACTIVE) * s as f64).floor() as usize;
            0.3 + 0.7 * nbtv_cell(pattern, self.lines as usize, s, line, cell.min(s - 1)) as f64
        }
    }
}

/// The NBTV radio: a channel at the centre, AM envelope or the narrow FM discriminator, to an audio-rate video signal.
pub struct NbtvRadio {
    /// FM (else AM).
    pub fm: bool,
    /// The format.
    pub format: NbtvFormat,
    /// FM deviation, Hz.
    pub deviation: f64,
    dec: Option<Decimator<Complex32>>,
    vdec: Option<Decimator<f32>>,
    /// AM: the channel brought to the carrier and narrowed to the video band before the envelope is taken (less noise in the
    /// envelope, so less of its bias at the low carrier of the sync tips).
    vdec_c: Option<Decimator<Complex32>>,
    prev: Complex32,
    disc: sdrkit_algo_dsp::fm::Discriminator,
    a: Vec<Complex32>,
    a2: Vec<Complex32>,
    b: Vec<f32>,
    c: Vec<f32>,
    fs1: f64,
    level: f64,
    /// AM carrier offset: the phase steps summed, the estimate (Hz) and the mixer's phase.
    acc: (f64, f64, usize),
    offset: f64,
    mix: sdrkit_algo_dsp::nco::Nco,
}

impl Default for NbtvRadio {
    fn default() -> Self {
        NbtvRadio {
            fm: false,
            format: NbtvFormat::default(),
            deviation: 5e3,
            dec: None,
            vdec: None,
            vdec_c: None,
            prev: Complex32::default(),
            disc: crate::util::phase_steps(),
            a: Vec::new(),
            a2: Vec::new(),
            b: Vec::new(),
            c: Vec::new(),
            fs1: 1.0,
            level: -100.0,
            acc: (0.0, 0.0, 0),
            offset: 0.0,
            mix: sdrkit_algo_dsp::nco::Nco::new(1.0, 0.0),
        }
    }
}

impl NbtvRadio {
    /// The rate of the video it outputs for input rate `fs`.
    pub fn rates(&self, fs: f64) -> (usize, f64, usize, f64) {
        let vbw = self.format.video_hz();
        let half = vbw + 60e3 + if self.fm { self.deviation } else { 0.0 };
        let d1 = ((fs / (2.5 * half)).floor() as usize).max(1);
        let fs1 = fs / d1 as f64;
        let d2 = ((fs1 / (4.0 * vbw)).floor() as usize).max(1);
        (d1, fs1, d2, fs1 / d2 as f64)
    }
}

impl Component for NbtvRadio {
    fn values(&self) -> sdrkit_core::Values {
        sdrkit_graph::no_values()
    }
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "atv.nbtv_radio",
            vec![Port::iq("in")],
            vec![Port::real("out"), Port::typed::<NbtvLevel>("level")],
        )
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let fs = inputs[0].rate;
        let vbw = self.format.video_hz();
        let half = vbw + 60e3 + if self.fm { self.deviation } else { 0.0 };
        let (d1, fs1, d2, fo) = self.rates(fs);
        self.fs1 = fs1;
        self.dec = Some(Decimator::new(fs, d1, half.min(0.45 * fs1), 60.0));
        self.vdec = Some(Decimator::new(fs1, d2, (1.2 * vbw).min(0.4 * fo), 50.0));
        self.vdec_c = Some(Decimator::new(fs1, d2, (1.2 * vbw).min(0.4 * fo), 50.0));
        self.prev = Complex32::default();
        self.disc.reset();
        self.mix = sdrkit_algo_dsp::nco::Nco::new(fs1, 0.0);
        self.acc = (0.0, 0.0, 0);
        self.offset = 0.0;
        let mut f = Format::new(Kind::Real, fo);
        f.bandwidth_hz = vbw;
        Ok(vec![f, Format::new(Kind::typed::<NbtvLevel>(), 0.0)])
    }
    fn process(&mut self, io: &mut Io) {
        let x = io.iq(0);
        self.a.clear();
        self.dec
            .as_mut()
            .expect("configure builds the decimators before process runs")
            .process(x, &mut self.a);
        self.b.clear();
        let mut p = 0.0f64;
        let k = self.fs1 / TAU / self.deviation;
        if self.fm {
            for &s in &self.a {
                p += s.norm_sqr() as f64;
                let ph = self.disc.process_sample(s) as f64;
                self.b.push(((ph * k + 1.0) / 2.0) as f32);
            }
            self.c.clear();
            self.vdec
                .as_mut()
                .expect("configure builds the decimators before process runs")
                .process(&self.b, &mut self.c);
        } else {
            // The carrier offset from the phase steps (the carrier is never below 15 %), judged every 20 ms; the channel is
            // mixed to it, narrowed to the video band, and its envelope taken.
            self.mix.set_freq(-self.offset);
            self.a2.clear();
            self.a2.extend_from_slice(&self.a);
            self.mix.mix_in_place(&mut self.a2);
            for &s in &self.a {
                p += s.norm_sqr() as f64;
                let d = s * self.prev.conj();
                self.prev = s;
                self.acc.0 += d.re as f64;
                self.acc.1 += d.im as f64;
                self.acc.2 += 1;
            }
            if self.acc.2 as f64 >= 0.02 * self.fs1 {
                let f = self.acc.1.atan2(self.acc.0) * self.fs1 / TAU;
                self.offset = if self.offset == 0.0 {
                    f
                } else {
                    self.offset + 0.3 * (f - self.offset)
                };
                self.acc = (0.0, 0.0, 0);
            }
            let mut z = std::mem::take(&mut self.a);
            z.clear();
            self.vdec_c
                .as_mut()
                .expect("configure builds the decimators before process runs")
                .process(&self.a2, &mut z);
            self.c.clear();
            self.c.extend(z.iter().map(|v| v.norm()));
            self.a = z;
        }
        if !self.a.is_empty() || !self.a2.is_empty() {
            let n = if self.fm { self.a.len() } else { self.a2.len() };
            if n > 0 {
                self.level += 0.05 * (10.0 * (p / n as f64).max(1e-20).log10() - self.level);
            }
        }
        io.real_out(0).extend_from_slice(&self.c);
        if io.tick() {
            let t = io.input_format(0).time_of(io.input(0).first());
            io.items_out::<NbtvLevel>(1).push(Timed {
                time: t,
                value: NbtvLevel(self.level),
            });
        }
    }
}

/// The NBTV carrier level, dBFS.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct NbtvLevel(pub f64);
impl sdrkit_graph::Item for NbtvLevel {
    const KIND: &'static str = "atv.nbtv_level";
}

/// The NBTV decoder core, usable on any real video signal (an audio file as well).
pub struct NbtvCore {
    /// The format.
    pub format: NbtvFormat,
    rate: f64,
    t: u64,
    period: f64,
    bottom: f64,
    top: f64,
    prev: f32,
    below: Option<f64>,
    next0h: Option<f64>,
    quality: f64,
    found_lines: u64,
    line_idx: u64,
    picture_sync: bool,
    pictures: u64,
    no_pic_sync_lines: u64,
    no_lines: u64,
    tried_pol: u32,
    /// The signal is turned over.
    pub inverted: bool,
    ring: crate::util::Ring<f32>,
    pending: Vec<(f64, bool)>,
    cells: Vec<f32>,
    rows_seen: u64,
    snr: f64,
    tip_var: f64,
    levels: Option<(f32, f32)>,
    line_scope: Vec<[f32; 2]>,
    /// Pictures made (grey, upscaled) waiting to be taken.
    pub out: Vec<Vec<u8>>,
    last_line_t: f64,
    log: Vec<String>,
}

impl NbtvCore {
    /// A decoder for `format` on video at `rate`.
    pub fn new(format: NbtvFormat, rate: f64) -> Self {
        let period = rate / format.line_hz();
        NbtvCore {
            format,
            rate,
            t: 0,
            period,
            bottom: 0.0,
            top: 0.0,
            prev: 0.0,
            below: None,
            next0h: None,
            quality: 0.0,
            found_lines: 0,
            line_idx: 0,
            picture_sync: false,
            pictures: 0,
            no_pic_sync_lines: 0,
            no_lines: 0,
            tried_pol: 0,
            inverted: false,
            ring: crate::util::ring((4.0 * period) as usize + 64),
            pending: Vec::new(),
            cells: vec![0.0; format.lines as usize * format.samples()],
            rows_seen: 0,
            snr: 0.0,
            tip_var: 0.0,
            levels: None,
            line_scope: Vec::new(),
            out: Vec::new(),
            last_line_t: 0.0,
            log: Vec::new(),
        }
    }

    /// Line locked.
    pub fn line_locked(&self) -> bool {
        self.quality > 0.5
    }
    /// Picture locked.
    pub fn picture_locked(&self) -> bool {
        self.line_locked() && self.picture_sync
    }
    /// Pictures made.
    pub fn pictures(&self) -> u64 {
        self.pictures
    }

    fn flip(&mut self) {
        self.inverted = !self.inverted;
        let (b, t) = (self.bottom, self.top);
        self.bottom = -t;
        self.top = -b;
        self.next0h = None;
        self.quality = 0.0;
        self.no_lines = 0;
        self.no_pic_sync_lines = 0;
        self.log.push(if self.inverted {
            "NBTV: the signal is turned over".into()
        } else {
            "NBTV: the signal is upright".into()
        });
    }

    /// Feeds video samples.
    pub fn push(&mut self, x: &[f32]) {
        let sync_len = SYNC * self.period;
        let w_decay = 1.0 / (3.0 * self.period);
        for &raw in x {
            let v = if self.inverted { -raw } else { raw } as f64;
            self.ring.push(v as f32);
            let now = self.t as f64;
            self.t += 1;
            if self.t == 1 {
                self.bottom = v;
                self.top = v;
            }
            if v < self.bottom {
                self.bottom = v;
            } else {
                self.bottom += w_decay * (self.top - self.bottom) * 0.05;
            }
            if v > self.top {
                self.top = v;
            } else {
                self.top -= w_decay * (self.top - self.bottom) * 0.05;
            }
            let th = (self.bottom + 0.15 * (self.top - self.bottom)) as f32;
            let vf = v as f32;
            if self.prev >= th && vf < th {
                let t = now - 1.0 + ((self.prev - th) / (self.prev - vf)) as f64;
                self.below = Some(t);
            } else if self.prev < th && vf >= th {
                if let Some(s) = self.below.take() {
                    let width = now - s;
                    if width > 0.5 * sync_len && width < 4.5 * sync_len {
                        self.pulse(s, width > 2.0 * sync_len);
                    }
                }
            }
            self.prev = vf;
            if let Some(n0) = self.next0h {
                if now > n0 + 0.12 * self.period
                    && self.below.is_none_or(|b| b > n0 + 0.12 * self.period)
                {
                    self.line(n0, false, false);
                    self.next0h = Some(n0 + self.period);
                }
            }
            // Lines whose samples are all in.
            while let Some(&(t0, long)) = self.pending.first() {
                if now < t0 + self.period + 2.0 {
                    break;
                }
                self.pending.remove(0);
                self.cells_of(t0, long);
            }
        }
        // Polarity trials.
        if !self.line_locked() {
            self.no_lines += x.len() as u64;
            if self.no_lines as f64 > 2.0 * self.rate {
                self.flip();
            }
        } else {
            self.no_lines = 0;
            if !self.picture_sync && self.tried_pol < 2 {
                self.no_pic_sync_lines += x.len() as u64;
                if self.no_pic_sync_lines as f64 > 4.0 * self.rate / self.format.rate {
                    self.tried_pol += 1;
                    self.flip();
                }
            }
        }
    }

    fn pulse(&mut self, t: f64, long: bool) {
        match self.next0h {
            None => {
                self.next0h = Some(t + self.period);
                self.line(t, true, long);
            }
            Some(n0) => {
                let d = t - n0;
                if d.abs() < 0.12 * self.period {
                    let k = if self.found_lines < 20 { 0.3 } else { 0.08 };
                    let t0 = n0 + k * d;
                    self.period += 0.1 * k * d;
                    let nominal = self.rate / self.format.line_hz();
                    self.period = self.period.clamp(0.97 * nominal, 1.03 * nominal);
                    self.next0h = Some(t0 + self.period);
                    self.line(t0, true, long);
                } else if d < -0.12 * self.period && d > -0.5 * self.period {
                    // Early pulse: ignore (picture content).
                } else if self.quality < 0.3 {
                    self.next0h = Some(t + self.period);
                    self.line(t, true, long);
                }
            }
        }
    }

    fn line(&mut self, t0: f64, found: bool, long: bool) {
        self.quality += 0.05 * ((found as u8 as f64) - self.quality);
        if found {
            self.found_lines += 1;
        }
        if long {
            if !self.picture_sync {
                self.log.push("NBTV: picture sync found".into());
            }
            self.picture_sync = true;
            self.no_pic_sync_lines = 0;
            if !self.line_idx.is_multiple_of(self.format.lines as u64) {
                self.line_idx = 0;
            }
        }
        self.last_line_t = t0;
        self.pending.push((t0, long));
    }

    fn cells_of(&mut self, t0: f64, long: bool) {
        let p = self.period;
        let tip_a = (t0 + 0.2 * SYNC * p).round() as i64;
        let tip_b = (t0 + 0.8 * SYNC * p).round() as i64;
        let (tip, tvar) = crate::util::mean_var(&self.ring, tip_a, tip_b);
        let porch0 = if long { 3.0 * SYNC } else { SYNC };
        let bl_a = (t0 + (porch0 + 0.2 * PORCH) * p).round() as i64;
        let bl_b = (t0 + (porch0 + 0.8 * PORCH) * p).round() as i64;
        let black = self.ring.mean_of(bl_a, bl_b);
        // The levels of a few lines together: a porch of a few samples is noisy on its own. A step (a level change, an AC
        // coupled source) is followed within a few lines.
        let (tip, black) = match self.levels {
            Some((t, b)) if ((black - tip) - (b - t)).abs() < 0.5 * (b - t).abs() => {
                let (t, b) = (t + 0.25 * (tip - t), b + 0.25 * (black - b));
                self.levels = Some((t, b));
                (t, b)
            }
            _ => {
                self.levels = Some((tip, black));
                (tip, black)
            }
        };
        let gain = ((black - tip) / 0.3).max(1e-6);
        self.tip_var += 0.05 * (tvar as f64 - self.tip_var);
        self.snr = 20.0 * (gain as f64 / self.tip_var.max(1e-20).sqrt()).log10();
        let lines = self.format.lines as usize;
        let s = self.format.samples();
        let li = (self.line_idx % lines as u64) as usize;
        for c in 0..s {
            let a = t0 + (SYNC + PORCH + ACTIVE * c as f64 / s as f64) * p;
            let b = t0 + (SYNC + PORCH + ACTIVE * (c + 1) as f64 / s as f64) * p;
            let hidden = long && a < t0 + (3.0 * SYNC + PORCH) * p;
            let v = if hidden {
                f32::NAN
            } else {
                let m = self.ring.mean_of(a.round() as i64, b.round() as i64);
                (((m - tip) / gain - 0.3) / 0.7).clamp(0.0, 1.0)
            };
            self.cells[li * s + c] = v;
        }
        if li == 10 || self.line_scope.is_empty() {
            self.line_scope.clear();
            for i in 0..256 {
                let k = t0 + p * i as f64 / 256.0;
                let v = (self.ring.at(k.round() as i64) - tip) / gain;
                self.line_scope.push([i as f32 / 256.0, v]);
            }
        }
        self.line_idx += 1;
        self.rows_seen += 1;
        if self.line_idx.is_multiple_of(lines as u64) && self.rows_seen >= lines as u64 {
            self.make_picture();
        }
    }

    fn make_picture(&mut self) {
        let lines = self.format.lines as usize;
        let s = self.format.samples();
        // Cells hidden by the long pulse: from the neighbouring line.
        for li in 0..lines {
            for c in 0..s {
                if self.cells[li * s + c].is_nan() {
                    let n = (li + 1) % lines;
                    self.cells[li * s + c] = self.cells[n * s + c];
                }
            }
        }
        let (w, h) = if self.format.vertical {
            (lines, s)
        } else {
            (s, lines)
        };
        let scale = (480 / h.max(w)).max(1);
        let (pw, ph) = (w * scale, h * scale);
        let mut px = vec![255u8; pw * ph * 4];
        for y in 0..ph {
            for x in 0..pw {
                let (cx, cy) = (x / scale, y / scale);
                let (li, c) = if self.format.vertical {
                    (cx, cy)
                } else {
                    (cy, cx)
                };
                let g = (self.cells[li * s + c].clamp(0.0, 1.0) * 255.0).round() as u8;
                let i = (y * pw + x) * 4;
                px[i] = g;
                px[i + 1] = g;
                px[i + 2] = g;
            }
        }
        self.out.push(px);
        self.pictures += 1;
    }

    /// The picture size (upscaled).
    pub fn picture_size(&self) -> (usize, usize) {
        let lines = self.format.lines as usize;
        let s = self.format.samples();
        let (w, h) = if self.format.vertical {
            (lines, s)
        } else {
            (s, lines)
        };
        let scale = (480 / h.max(w)).max(1);
        (w * scale, h * scale)
    }

    /// The cell values (0..1), line by line.
    pub fn cells(&self) -> &[f32] {
        &self.cells
    }

    /// Line rate, Hz.
    pub fn line_hz(&self) -> f64 {
        self.rate / self.period
    }

    fn report(&mut self, level: f64) -> NbtvReport {
        let lh = self.line_hz();
        NbtvReport {
            line_locked: self.line_locked(),
            picture_locked: self.picture_locked(),
            line_hz: lh,
            line_ppm: (lh / self.format.line_hz() - 1.0) * 1e6,
            quality: self.quality,
            lines: self.found_lines,
            pictures: self.pictures,
            snr_db: self.snr.clamp(0.0, 60.0),
            line: self.line_scope.clone(),
            name: self.format.name(),
            level_dbfs: level,
            inverted: self.inverted,
            log: std::mem::take(&mut self.log),
        }
    }
}

/// The NBTV decoder component.
pub struct NbtvDecoder {
    /// The format.
    pub format: NbtvFormat,
    core: Option<NbtvCore>,
    level: f64,
}

impl Default for NbtvDecoder {
    fn default() -> Self {
        NbtvDecoder {
            format: NbtvFormat::default(),
            core: None,
            level: -100.0,
        }
    }
}

impl Component for NbtvDecoder {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "atv.nbtv",
            vec![
                Port::real("in"),
                Port::typed::<NbtvLevel>("level"),
                Port::typed::<Tune>("control"),
            ],
            vec![
                Port::typed::<Picture>("out").rate_from(RateFrom::Fixed(0.0)),
                Port::typed::<NbtvReport>("report").rate_from(RateFrom::Fixed(0.0)),
            ],
        )
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.core = Some(NbtvCore::new(self.format, inputs[0].rate));
        Ok(vec![
            Format::new(Kind::typed::<Picture>(), 0.0),
            Format::new(Kind::typed::<NbtvReport>(), 0.0),
        ])
    }
    fn process(&mut self, io: &mut Io) {
        if let Some(l) = last::<NbtvLevel>(io, 1) {
            self.level = l.0;
        }
        let x = io.real(0);
        let core = self
            .core
            .as_mut()
            .expect("configure builds the core before process runs");
        core.push(x);
        let t = io.input_format(0).time_of(io.input(0).first());
        let (w, h) = core.picture_size();
        for px in core.out.drain(..) {
            let pic = Picture {
                codec: Codec::Pixels(PixelFormat::Bgra),
                layout: Some(PixelLayout::packed(PixelFormat::Bgra, w as u32, h as u32)),
                name: None,
                id: Picture::WHOLE,
                state: sdrkit_core::PictureState::Finished,
                data: Arc::from(px.into_boxed_slice()),
            };
            io.items_out::<Picture>(0).push(Timed {
                time: t,
                value: pic,
            });
        }
        if io.tick() {
            let rep = core.report(self.level);
            io.items_out::<NbtvReport>(1).push(Timed {
                time: t,
                value: rep,
            });
        }
    }
    fn values(&self) -> Values {
        no_values()
    }
}

/// The NBTV test signal on a carrier.
pub struct NbtvGenerator {
    format: NbtvFormat,
    rate: f64,
    fm: bool,
    deviation: f64,
    pattern: u32,
    level: f64,
    k: u64,
    phase: f64,
}

impl NbtvGenerator {
    /// A generator of `format` at `rate`: on narrow FM with a peak `deviation` (Hz) when `fm`, else on AM. `pattern` is the
    /// picture (0 to 2).
    pub fn new(format: NbtvFormat, rate: f64, fm: bool, deviation: f64, pattern: u32) -> Self {
        NbtvGenerator {
            format,
            rate,
            fm,
            deviation,
            pattern,
            level: 0.4,
            k: 0,
            phase: 0.0,
        }
    }
    /// The video (sync 0, black 0.3, white 1) at time `t`, each sample the mean of four points across its interval.
    pub fn video(format: &NbtvFormat, t: f64, dt: f64, pattern: u32) -> f64 {
        (0..4)
            .map(|j| format.level(t + (j as f64 + 0.5) / 4.0 * dt - dt / 2.0, pattern))
            .sum::<f64>()
            / 4.0
    }
}

impl Generator for NbtvGenerator {
    fn rate_hz(&self) -> Hz {
        self.rate
    }
    /// The stated power is the peak carrier power, in the bandwidth of twice the video band.
    fn signal_info(&self) -> Option<sdrkit_core::SignalInfo> {
        Some(
            sdrkit_core::SignalInfo::new(2.0 * self.format.video_hz())
                .with_power(self.level * self.level),
        )
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        let dt = 1.0 / self.rate;
        // The video is evaluated at a lower rate and held (it has a few kHz of bandwidth).
        let step = ((self.rate / (40.0 * self.format.video_hz())).floor() as u64).max(1);
        let mut v = 0.0;
        for (i, o) in out.iter_mut().enumerate() {
            let k = self.k + i as u64;
            if k.is_multiple_of(step) || i == 0 {
                v = Self::video(&self.format, k as f64 * dt, dt * step as f64, self.pattern);
            }
            *o = if self.fm {
                self.phase = (self.phase + self.deviation * (2.0 * v - 1.0) * dt).rem_euclid(1.0);
                let (sn, cs) = sdrkit_algo_dsp::approx::sin_cos(TAU * self.phase);
                Complex32::new(cs, sn) * self.level as f32
            } else {
                let a = self.level * (0.15 + 0.85 * v);
                Complex32::new(a as f32, 0.0)
            };
        }
        self.k += out.len() as u64;
        out.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_format_has_its_numbers() {
        let f = NbtvFormat::default();
        assert_eq!(f.line_hz(), 400.0);
        assert_eq!(f.samples(), 48);
        assert!((f.video_hz() - 10_666.7).abs() < 1.0);
        assert_eq!(f.name(), "NBTV 32 lines 12.5/s");
        assert_eq!(
            NbtvFormat {
                lines: 60,
                rate: 25.0,
                vertical: true
            }
            .name(),
            "mechanical 60 lines 25/s"
        );
        assert_eq!(f.level(0.0001, 0), 0.0);
        assert_eq!(f.level(1.0 / 400.0 + 0.0001, 0), 0.0);
        assert!((f.level(1.0 / 400.0 + 0.00017, 0) - 0.3).abs() < 1e-9);
    }

    #[test]
    fn the_core_decodes_a_clean_audio_signal() {
        let f = NbtvFormat::default();
        let rate = 48_000.0;
        let n = (3.0 * rate) as usize;
        let x: Vec<f32> = (0..n)
            .map(|i| NbtvGenerator::video(&f, i as f64 / rate, 1.0 / rate, 0) as f32)
            .collect();
        let mut c = NbtvCore::new(f, rate);
        c.push(&x);
        assert!(
            c.line_locked() && c.picture_locked(),
            "{} {}",
            c.quality,
            c.picture_sync
        );
        assert!(c.pictures() >= 20, "{}", c.pictures());
    }
}
