//! The test signal generator: a complete mode I ensemble at 2.048 Msps (and through resampling at any rate
//! from 2 to 21 Msps), with its channel coding written from the standard independently of the receiver.
//!
//! The coded content of the 12 s loop (every sub-channel's CIFs) is computed once; the FIC is made per frame (its CIF counter and time
//! run on); the generator is endless.

pub mod content;
pub mod data;
pub mod fic;
pub mod png;
pub mod services;

use crate::settings;
use crate::tables::{self, Protection, CARRIERS, CIF_BITS, FRAME, SYMBOL_BITS, TG, TNULL, TU};
use content::Programme;
use sdrkit_algo_dsp::fft::Fft;
use sdrkit_algo_dsp::resample::FractionalResampler;
use sdrkit_core::{Complex32 as C, GenConfig, Generator, Hz, SignalInfo};
use sdrkit_util_bits::interleave::DelayLanes;
use sdrkit_util_bits::pack::Pad;
use sdrkit_util_bits::seq::PRBS9_DISPERSAL;
use sdrkit_util_bits::BitOrder;
use services::{PadContent, LOOP_FRAMES};

/// What a generated service carries.
#[derive(Clone, Debug, PartialEq)]
pub enum ServiceAudio {
    /// DAB+ AAC-LC at this rate (48 000 or 32 000).
    DabPlus {
        /// Sampling rate.
        rate: u32,
    },
    /// MPEG Layer II at this rate (48 000 or 24 000), stereo or mono.
    Mp2 {
        /// Sampling rate.
        rate: u32,
        /// Two channels.
        stereo: bool,
    },
    /// Stream or packet data supplied as logical frames (each bit rate × 3 bytes; repeated over the loop).
    Data {
        /// TMId 1 (stream) or 3 (packet).
        tmid: u8,
        /// DSCTy.
        dscty: u8,
        /// SCId (packet mode).
        scid: u16,
        /// The logical frames.
        frames: Vec<Vec<u8>>,
    },
}

/// One generated service.
#[derive(Clone, Debug)]
pub struct ServiceSpec {
    /// SId.
    pub sid: u32,
    /// Label.
    pub label: String,
    /// Sub-channel id.
    pub subch: u8,
    /// Bit rate in kbit/s.
    pub bitrate: u16,
    /// Protection.
    pub protection: Protection,
    /// What it carries.
    pub audio: ServiceAudio,
    /// The programme.
    pub programme: Programme,
    /// PAD content (DAB+ services).
    pub pad: PadContent,
    /// Conditional access: CAId 1 and the CA flag (the audio stays clear).
    pub ca: bool,
    /// Further components of the service in FIG 0/2: (TMId, ASCTy or DSCTy, sub-channel or SCId).
    pub extra_components: Vec<(u8, u8, u16)>,
}

/// The announcement the generator signals.
#[derive(Clone, Debug, PartialEq)]
pub struct AnnouncementSpec {
    /// Cluster id.
    pub cluster: u8,
    /// The types every audio service supports (ASu flags).
    pub support: u16,
    /// The types announced now (ASw flags; 0 for none).
    pub active: u16,
    /// The sub-channel that carries it.
    pub subch: u8,
}

/// Everything the generator is made with.
#[derive(Clone, Debug)]
pub struct GenParams {
    /// Ensemble label.
    pub label: String,
    /// EId.
    pub eid: u16,
    /// Services.
    pub services: Vec<ServiceSpec>,
    /// Output rate.
    pub rate_hz: f64,
    /// Time of frame 0, seconds since 1970.
    pub t0_s: f64,
    /// Programme type for every audio service: (type, dynamic).
    pub pty: Option<(u8, bool)>,
    /// Announcements.
    pub announcement: Option<AnnouncementSpec>,
    /// Labels as FIG 2 (UTF-8) instead of FIG 1.
    pub fig2_labels: bool,
    /// Transmitters to identify in the null symbol: (MainId, SubId, level in dB).
    pub tii: Vec<(u8, u8, f64)>,
    /// Extra FIGs (with their header byte), sent every frame.
    pub extra_figs: Vec<Vec<u8>>,
}

impl GenParams {
    /// The four default services, all EEP 3-A.
    pub fn default_services() -> Vec<ServiceSpec> {
        let eep = Protection::Eep {
            option: 0,
            level: 3,
        };
        let slide = png::test_slide();
        vec![
            ServiceSpec {
                sid: 0xC101,
                label: "Test Tones 1".into(),
                subch: 1,
                bitrate: 48,
                protection: eep,
                audio: ServiceAudio::DabPlus { rate: 48_000 },
                programme: Programme::Tones(1000.0, 3000.0),
                pad: PadContent::default(),
                ca: false,
                extra_components: Vec::new(),
            },
            ServiceSpec {
                sid: 0xC102,
                label: "Test Tones 2".into(),
                subch: 2,
                bitrate: 32,
                protection: eep,
                audio: ServiceAudio::DabPlus { rate: 32_000 },
                programme: Programme::Tones(2000.0, 500.0),
                pad: PadContent::default(),
                ca: false,
                extra_components: Vec::new(),
            },
            ServiceSpec {
                sid: 0xC103,
                label: "Test Melody".into(),
                subch: 3,
                bitrate: 64,
                protection: eep,
                audio: ServiceAudio::DabPlus { rate: 48_000 },
                programme: Programme::Melody,
                pad: PadContent {
                    label: Some("Test Melody - test tune".into()),
                    slide: Some(("Test slide".into(), slide)),
                    dl_plus: Vec::new(),
                    next_slide: None,
                },
                ca: false,
                extra_components: Vec::new(),
            },
            ServiceSpec {
                sid: 0xC104,
                label: "Test MP2".into(),
                subch: 4,
                bitrate: 128,
                protection: eep,
                audio: ServiceAudio::Mp2 {
                    rate: 48_000,
                    stereo: true,
                },
                programme: Programme::Tones(1500.0, 750.0),
                pad: PadContent::default(),
                ca: false,
                extra_components: Vec::new(),
            },
        ]
    }
}

impl Default for GenParams {
    fn default() -> Self {
        GenParams {
            label: "Test Ensemble".into(),
            eid: 0xC0DE,
            services: Self::default_services(),
            rate_hz: tables::RATE,
            t0_s: 1_700_000_000.0,
            pty: None,
            announcement: None,
            fig2_labels: false,
            tii: Vec::new(),
            extra_figs: Vec::new(),
        }
    }
}

/// A sub-channel as laid out.
#[derive(Clone, Debug)]
pub struct Layout {
    /// Sub-channel id.
    pub subch: u8,
    /// Start CU.
    pub start: u16,
    /// Size CU.
    pub size: u16,
    /// Protection.
    pub protection: Protection,
    /// Bit rate.
    pub bitrate: u16,
}

/// The coded loop: every CIF of the 12 s (500 CIFs of 55 296 bits, packed) and the logical frames sent.
pub struct CodedLoop {
    /// Packed CIF bits.
    pub cifs: Vec<Vec<u8>>,
    /// The layout.
    pub layout: Vec<Layout>,
    /// The logical frames of each service, by sub-channel.
    pub frames: Vec<(u8, Vec<Vec<u8>>)>,
}

/// Codes the services' logical frames into the 500 CIFs of the loop. Services that do not fit are left out.
pub fn code_loop(p: &GenParams) -> CodedLoop {
    let mut layout = Vec::new();
    let mut frames = Vec::new();
    let mut cu = 0u16;
    for s in &p.services {
        let Some(prof) = tables::profile(s.protection, s.bitrate as usize) else {
            continue;
        };
        let size = prof.size_cu as u16;
        if cu as usize + size as usize > tables::CU_PER_CIF {
            continue;
        }
        let lf = match &s.audio {
            ServiceAudio::DabPlus { rate } => {
                services::dabplus_frames(s.bitrate, *rate, s.programme, &s.pad)
            }
            ServiceAudio::Mp2 { rate, stereo } => {
                services::mp2_frames(s.bitrate, *rate, *stereo, s.programme)
            }
            ServiceAudio::Data { frames, .. } => {
                let n = s.bitrate as usize * 3;
                (0..LOOP_FRAMES)
                    .map(|i| {
                        let mut f = frames
                            .get(i % frames.len().max(1))
                            .cloned()
                            .unwrap_or_default();
                        f.resize(n, 0);
                        f
                    })
                    .collect()
            }
        };
        layout.push(Layout {
            subch: s.subch,
            start: cu,
            size,
            protection: s.protection,
            bitrate: s.bitrate,
        });
        frames.push((s.subch, lf));
        cu += size;
    }
    // Code every logical frame.
    let mut coded: Vec<Vec<Vec<u8>>> = Vec::new();
    for (l, (_, lfs)) in layout.iter().zip(&frames) {
        let prof = tables::profile(l.protection, l.bitrate as usize).expect("checked");
        let bits = l.size as usize * 64;
        let c: Vec<Vec<u8>> = lfs
            .iter()
            .map(|f| {
                let mut out = Vec::with_capacity(bits);
                crate::fec::encode_profile(&prof, f, &mut out);
                out.resize(bits, 0);
                out
            })
            .collect();
        coded.push(c);
    }
    // Time interleaving over the loop: the frames are sent twice through the interleaver and the second pass is kept, so the
    // loop repeats without a gap.
    let mut interleaved: Vec<Vec<Vec<u8>>> = Vec::new();
    for (l, c) in layout.iter().zip(&coded) {
        let n = l.size as usize * 64;
        let mut lanes = DelayLanes::dab_time(n as u32, false, 0u8).expect("a whole number of CUs");
        let mut out = Vec::with_capacity(LOOP_FRAMES);
        for pass in 0..2 {
            for f in c.iter().take(LOOP_FRAMES) {
                let mut x = f.clone();
                lanes.process(&mut x);
                if pass == 1 {
                    out.push(x);
                }
            }
        }
        interleaved.push(out);
    }
    let mut cifs = Vec::with_capacity(LOOP_FRAMES);
    let mut bits = vec![0u8; CIF_BITS];
    for j in 0..LOOP_FRAMES {
        // Unused capacity: the dispersal sequence offset by 37 × (CIF number mod 511).
        let mut pr = PRBS9_DISPERSAL;
        pr.skip(((37 * (j % 511)) % 511) as u64);
        pr.fill_bits(&mut bits);
        for (l, c) in layout.iter().zip(&interleaved) {
            let a = l.start as usize * 64;
            bits[a..a + l.size as usize * 64].copy_from_slice(&c[j]);
        }
        let mut packed = Vec::with_capacity(CIF_BITS / 8);
        sdrkit_util_bits::pack::pack(&bits, BitOrder::Msb, Pad::Zero, &mut packed);
        cifs.push(packed);
    }
    CodedLoop {
        cifs,
        layout,
        frames,
    }
}

/// The 70 TII patterns (EN 300 401 table 26): the 8-bit words with four ones in increasing order, b = 0 the most significant.
pub fn tii_patterns() -> Vec<[bool; 8]> {
    (0u16..256)
        .filter(|v| v.count_ones() == 4)
        .map(|v| {
            let mut p = [false; 8];
            for (b, x) in p.iter_mut().enumerate() {
                *x = (v >> (7 - b)) & 1 == 1;
            }
            p
        })
        .collect()
}

/// The carriers k with A_{c,p}(k) = 1 of a transmitter (the first carrier of each pair; the pair is k, k + 1).
pub fn tii_carriers(main: u8, sub: u8) -> Vec<i32> {
    let pats = tii_patterns();
    let Some(p) = pats.get(main as usize) else {
        return Vec::new();
    };
    let c = sub as i32;
    let mut v = Vec::new();
    for (b, &on) in p.iter().enumerate() {
        if !on {
            continue;
        }
        let b = b as i32;
        v.push(-768 + 2 * c + 48 * b);
        v.push(-384 + 2 * c + 48 * b);
        v.push(1 + 2 * c + 48 * b);
        v.push(385 + 2 * c + 48 * b);
    }
    v
}

/// The OFDM modulator: frames of 196 608 samples from the FIC and MSC bits.
pub struct Modulator {
    ifft: Fft,
    prs: Vec<C>,
    interleaver: Vec<i32>,
    bins: Vec<C>,
    z: Vec<C>,
    scale: f32,
}

impl Default for Modulator {
    fn default() -> Self {
        Self::new()
    }
}

impl Modulator {
    /// A modulator.
    pub fn new() -> Self {
        Modulator {
            ifft: Fft::inverse(TU),
            prs: tables::prs_bins(),
            interleaver: tables::frequency_interleaver(),
            bins: vec![C::default(); TU],
            z: vec![C::default(); TU],
            // Unit mean power while transmitting: 1536 unit carriers through an unscaled inverse FFT.
            scale: 1.0 / (CARRIERS as f32).sqrt(),
        }
    }

    fn symbol(&mut self, out: &mut Vec<C>, guard: usize) {
        self.bins.copy_from_slice(&self.z);
        self.ifft.process(&mut self.bins);
        let s = self.scale;
        for i in TU - guard..TU {
            out.push(clip(self.bins[i] * s));
        }
        for i in 0..TU {
            out.push(clip(self.bins[i] * s));
        }
    }

    /// One frame: the null (with TII carriers when given), the PRS and 75 symbols of `bits` (230 400 bits, one per
    /// byte: the FIC then the MSC).
    pub fn frame(&mut self, bits: &[u8], tii: &[(u8, u8, f64)], out: &mut Vec<C>) {
        // Null symbol.
        if tii.is_empty() {
            out.extend(std::iter::repeat_n(C::default(), TNULL));
        } else {
            self.z.iter_mut().for_each(|v| *v = C::default());
            for &(m, s, db) in tii {
                let a = 10f32.powf(db as f32 / 20.0);
                for k in tii_carriers(m, s) {
                    for kk in [k, k + 1] {
                        if kk != 0 && (-768..=768).contains(&kk) {
                            let b = tables::bin(kk);
                            self.z[b] += self.prs[b] * a;
                        }
                    }
                }
            }
            self.symbol(out, TNULL - TU);
        }
        self.z.copy_from_slice(&self.prs);
        self.symbol(out, TG);
        let r = std::f32::consts::FRAC_1_SQRT_2;
        for l in 0..75 {
            let b = &bits[l * SYMBOL_BITS..(l + 1) * SYMBOL_BITS];
            for (n, &k) in self.interleaver.iter().enumerate() {
                let y = C::new(
                    (1.0 - 2.0 * b[n] as f32) * r,
                    (1.0 - 2.0 * b[n + CARRIERS] as f32) * r,
                );
                let i = tables::bin(k);
                self.z[i] *= y;
            }
            self.symbol(out, TG);
        }
    }
}

/// The peak the modulator allows: 4.5 times the RMS level (13 dB), which keeps the clipping noise far below any decodable C/N.
const PEAK: f32 = 4.5;

fn clip(v: C) -> C {
    let m = v.norm();
    if m > PEAK {
        v * (PEAK / m)
    } else {
        v
    }
}

/// The ensemble generator.
pub struct DabGenerator {
    params: GenParams,
    coded: std::sync::Arc<CodedLoop>,
    modulator: Modulator,
    frame: u64,
    buf: Vec<C>,
    pos: usize,
    stages: Vec<FractionalResampler<C>>,
    stage_bufs: Vec<Vec<C>>,
    out: Vec<C>,
    out_pos: usize,
    label_figs: Vec<Vec<Vec<u8>>>,
}

impl DabGenerator {
    /// A generator from its parameters (codes the 12 s loop: a few seconds of work).
    pub fn new(params: GenParams) -> Self {
        let coded = std::sync::Arc::new(code_loop(&params));
        Self::from_loop(params, coded)
    }

    /// A generator from its parameters and an already coded loop (the services and layout of `params` must be those it was
    /// coded from; the rate, TII and FIC options can differ).
    pub fn from_loop(params: GenParams, coded: std::sync::Arc<CodedLoop>) -> Self {
        let mut stages = Vec::new();
        let total = params.rate_hz / tables::RATE;
        let needs = (params.rate_hz - tables::RATE).abs() > 0.5;
        if needs {
            let n = (total.ln() / 4f64.ln()).abs().ceil().max(1.0) as usize;
            let step = total.powf(1.0 / n as f64);
            let mut r = tables::RATE;
            for i in 0..n {
                let next = if i + 1 == n {
                    tables::RATE * total
                } else {
                    r * step
                };
                let lo = r.min(next);
                if let Ok(fr) = FractionalResampler::new(r, next, 0.385 * lo, 0.5 * lo, 70.0)
                    .or_else(|_| FractionalResampler::with_rates(r, next))
                {
                    stages.push(fr);
                }
                r = next;
            }
        }
        let label_figs = Self::labels(&params, &coded);
        let n_stages = stages.len();
        DabGenerator {
            params,
            coded,
            modulator: Modulator::new(),
            frame: 0,
            buf: Vec::with_capacity(FRAME),
            pos: 0,
            stages,
            stage_bufs: vec![Vec::new(); n_stages],
            out: Vec::new(),
            out_pos: 0,
            label_figs,
        }
    }

    /// From the stack's settings (the test signal section of the schema).
    pub fn from_config(cfg: &GenConfig) -> Self {
        let mut p = GenParams {
            rate_hz: cfg.rate_hz,
            ..Default::default()
        };
        let v = &cfg.settings;
        if v.get(&settings::GEN_TII) {
            p.tii = vec![(1, 1, 0.0), (1, 3, -5.0), (1, 7, -11.0)];
        }
        if v.get(&settings::GEN_FIG2) {
            p.fig2_labels = true;
        }
        let pty = v.get(&settings::GEN_PTY);
        if pty > 0 {
            p.pty = Some((pty as u8, false));
        }
        DabGenerator::new(p)
    }

    /// The layout of the sub-channels.
    pub fn layout(&self) -> &[Layout] {
        &self.coded.layout
    }

    /// The logical frames of a sub-channel's loop.
    pub fn logical_frames(&self, subch: u8) -> Option<&[Vec<u8>]> {
        self.coded
            .frames
            .iter()
            .find(|f| f.0 == subch)
            .map(|f| f.1.as_slice())
    }

    /// The parameters.
    pub fn params(&self) -> &GenParams {
        &self.params
    }

    fn labels(p: &GenParams, coded: &CodedLoop) -> Vec<Vec<Vec<u8>>> {
        let mut v = Vec::new();
        let mk = |ext: u8, id: Vec<u8>, label: &str| -> Vec<Vec<u8>> {
            if p.fig2_labels {
                fic::fig2_segments(ext, &id, label, false, 0xFF00)
            } else {
                vec![fic::fig1(ext, &id, label, 0xFF00)]
            }
        };
        v.push(mk(0, p.eid.to_be_bytes().to_vec(), &p.label));
        for s in p
            .services
            .iter()
            .filter(|s| coded.layout.iter().any(|l| l.subch == s.subch))
        {
            if s.sid > 0xFFFF {
                v.push(mk(5, s.sid.to_be_bytes().to_vec(), &s.label));
            } else {
                v.push(mk(1, (s.sid as u16).to_be_bytes().to_vec(), &s.label));
            }
        }
        v
    }

    /// The 12 FIBs of frame `m`.
    pub fn fibs(&self, m: u64) -> Vec<[u8; 32]> {
        let p = &self.params;
        let present: Vec<&ServiceSpec> = p
            .services
            .iter()
            .filter(|s| self.coded.layout.iter().any(|l| l.subch == s.subch))
            .collect();
        let cif = (4 * m) % 5000;
        let first = vec![
            fic::fig0_0(p.eid, cif as u16),
            fic::fig0_7(present.len() as u8, 0),
            fic::fig0_10(p.t0_s + 0.096 * m as f64),
        ];
        let mut rest: Vec<Vec<Vec<u8>>> = Vec::new();
        for chunk in self.coded.layout.chunks(7) {
            let e: Vec<(u8, u16, Protection, u16)> = chunk
                .iter()
                .map(|l| (l.subch, l.start, l.protection, l.size))
                .collect();
            rest.push(vec![fic::fig0_1(&e)]);
        }
        let to_fig02 = |s: &ServiceSpec| {
            let (tmid, ty, sc) = match &s.audio {
                ServiceAudio::DabPlus { .. } => (0, 63, s.subch as u16),
                ServiceAudio::Mp2 { .. } => (0, 0, s.subch as u16),
                ServiceAudio::Data {
                    tmid, dscty, scid, ..
                } => (
                    *tmid,
                    *dscty,
                    if *tmid == 3 { *scid } else { s.subch as u16 },
                ),
            };
            let mut components = vec![(tmid, ty, sc, true, s.ca)];
            components.extend(
                s.extra_components
                    .iter()
                    .map(|&(t, y, c)| (t, y, c, false, false)),
            );
            fic::Fig02Service {
                sid: s.sid,
                caid: if s.ca { 1 } else { 0 },
                components,
            }
        };
        let prog: Vec<&ServiceSpec> = present
            .iter()
            .copied()
            .filter(|s| s.sid <= 0xFFFF)
            .collect();
        let data: Vec<&ServiceSpec> = present.iter().copied().filter(|s| s.sid > 0xFFFF).collect();
        for chunk in prog.chunks(5) {
            rest.push(vec![fic::fig0_2(
                &chunk.iter().map(|s| to_fig02(s)).collect::<Vec<_>>(),
                false,
            )]);
        }
        for chunk in data.chunks(2) {
            rest.push(vec![fic::fig0_2(
                &chunk.iter().map(|s| to_fig02(s)).collect::<Vec<_>>(),
                true,
            )]);
        }
        let audio: Vec<&ServiceSpec> = prog
            .iter()
            .copied()
            .filter(|s| !matches!(s.audio, ServiceAudio::Data { .. }))
            .collect();
        if let Some((pty, dynamic)) = p.pty {
            for chunk in audio.chunks(5) {
                let e: Vec<(u16, bool, Option<u8>, u8)> = chunk
                    .iter()
                    .map(|s| (s.sid as u16, dynamic, Some(9), pty))
                    .collect();
                rest.push(vec![fic::fig0_17(&e)]);
            }
        }
        if let Some(a) = &p.announcement {
            for chunk in audio.chunks(5) {
                let e: Vec<(u16, u16, Vec<u8>)> = chunk
                    .iter()
                    .map(|s| (s.sid as u16, a.support, vec![a.cluster]))
                    .collect();
                rest.push(vec![fic::fig0_18(&e)]);
            }
            if a.active != 0 {
                rest.push(vec![fic::fig0_19(&[(a.cluster, a.active, a.subch)])]);
            }
        }
        for f in &p.extra_figs {
            rest.push(vec![f.clone()]);
        }
        let n = self.label_figs.len() as u64;
        for k in 0..3u64 {
            let i = ((3 * m + k) % n.max(1)) as usize;
            if let Some(g) = self.label_figs.get(i) {
                rest.push(g.clone());
            }
        }
        fic::pack(&first, &rest)
    }

    /// The baseband of frame `m` at 2.048 Msps.
    pub fn clean_frame(&mut self, m: u64, out: &mut Vec<C>) {
        let fibs = self.fibs(m);
        let mut bits = fic::encode_fic(&fibs);
        for c in 0..4 {
            let j = ((4 * m + c) % LOOP_FRAMES as u64) as usize;
            sdrkit_util_bits::pack::unpack(&self.coded.cifs[j], BitOrder::Msb, &mut bits);
        }
        // TII in the frames comprising CIF counts 0 … 3 modulo 8.
        let tii = if (4 * m) % 8 < 4 {
            self.params.tii.clone()
        } else {
            Vec::new()
        };
        self.modulator.frame(&bits, &tii, out);
    }

    fn next_frame(&mut self) {
        self.buf.clear();
        let m = self.frame;
        let mut tmp = std::mem::take(&mut self.buf);
        self.clean_frame(m, &mut tmp);
        self.buf = tmp;
        self.frame += 1;
        self.pos = 0;
    }
}

impl Generator for DabGenerator {
    fn rate_hz(&self) -> Hz {
        self.params.rate_hz
    }

    /// The ensemble occupies 1.536 MHz. Its QPSK cells come at 1536 per symbol of 2552 samples. The mean power while it
    /// transmits is 1 (the null symbol is quiet).
    fn signal_info(&self) -> Option<SignalInfo> {
        Some(SignalInfo {
            bandwidth_hz: CARRIERS as f64 * 1000.0,
            symbol_rate_hz: Some(CARRIERS as f64 * tables::RATE / tables::TS as f64),
            bit_rate_hz: None,
            power: 1.0,
        })
    }

    fn generate(&mut self, out: &mut [C]) -> usize {
        let mut written = 0;
        while written < out.len() {
            if self.stages.is_empty() {
                if self.pos >= self.buf.len() {
                    self.next_frame();
                }
                let n = (self.buf.len() - self.pos).min(out.len() - written);
                out[written..written + n].copy_from_slice(&self.buf[self.pos..self.pos + n]);
                self.pos += n;
                written += n;
            } else {
                if self.out_pos >= self.out.len() {
                    self.next_frame();
                    let n = self.stages.len();
                    for i in 0..n {
                        let (done, rest) = self.stage_bufs.split_at_mut(i);
                        let input: &[C] = if i == 0 { &self.buf } else { &done[i - 1] };
                        rest[0].clear();
                        self.stages[i].process(input, &mut rest[0]);
                    }
                    std::mem::swap(&mut self.out, &mut self.stage_bufs[n - 1]);
                    self.out_pos = 0;
                }
                let n = (self.out.len() - self.out_pos).min(out.len() - written);
                out[written..written + n]
                    .copy_from_slice(&self.out[self.out_pos..self.out_pos + n]);
                self.out_pos += n;
                written += n;
            }
        }
        written
    }
}
