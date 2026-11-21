//! The voice channel receiver: one carrier at 48 kHz in, gated audio in 10 ms blocks out. It holds the channel filter, the
//! FM, AM, USB and LSB demodulators, the squelch with its look-ahead, the choice of the demodulator from the signal, and the
//! reader of the sub-audible tone (CTCSS and DCS of TIA-603, with the shared decoders).

use sdrkit_algo_dsp::fm::Discriminator;
use sdrkit_algo_dsp::iir::{Biquad, BiquadCoeffs, Butterworth4};
use sdrkit_core::Complex32;
use sdrkit_proto_channels::{AUDIO_RATE, BLOCK};
use sdrkit_proto_tones::components::SubAudioEvent;
use sdrkit_proto_tones::ctcss::{CtcssConfig, CtcssDecoder, CtcssEventKind};
use sdrkit_proto_tones::dcs::{DcsConfig, DcsDecoder, DcsEventKind};
use sdrkit_util_ids::tones::ctcss::TONES;
use std::collections::VecDeque;
use std::f64::consts::TAU;

sdrkit_core::choice_enum! {
    /// A demodulator.
    pub enum Demod {
        /// Narrow FM.
        Fm = ("fm", "FM"),
        /// AM.
        Am = ("am", "AM"),
        /// Upper sideband.
        Usb = ("usb", "USB"),
        /// Lower sideband.
        Lsb = ("lsb", "LSB"),
    }
}

/// The FM squelch threshold of squelch setting `percent`, dB.
pub fn fm_threshold_db(percent: f64) -> f64 {
    1.0 + 0.08 * percent
}

/// The threshold of the carrier finder for squelch setting `percent`, dB above the floor of a cell.
pub fn finder_threshold_db(percent: f64) -> f64 {
    5.0 + 0.08 * percent
}

/// The label of a CTCSS tone: "88.5 Hz".
pub fn tone_label(hz: f32) -> String {
    format!("{hz:.1} Hz")
}

/// The number a handset gives a CTCSS tone: its place (1 to 38) among the 38 standard tones in rising order, 0 for another
/// tone.
pub fn handset_number(hz: f32) -> u32 {
    TONES
        .iter()
        .filter(|t| t.eia)
        .position(|t| (t.hz - hz).abs() < 0.05)
        .map_or(0, |i| i as u32 + 1)
}

/// The settings of a [`VoiceRx`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RxConfig {
    /// The cut-off of the channel filter, Hz (3000 at least).
    pub cutoff_hz: f64,
    /// The demodulator of the plan. `None` chooses AM, FM or SSB from the signal.
    pub plan: Option<Demod>,
    /// The demodulator the user forces.
    pub forced: Option<Demod>,
    /// The squelch, 0 to 100 %.
    pub squelch: f64,
    /// 750 us de-emphasis of FM audio.
    pub deemphasis: bool,
    /// The FM measure opens the squelch. False for HF voice, where only the carrier finder does.
    pub fm_squelch: bool,
    /// Read the sub-audible tone.
    pub tones: bool,
}

impl RxConfig {
    fn uses(&self, d: Demod) -> bool {
        self.forced.or(self.plan).is_none_or(|x| x == d)
    }
    fn auto(&self) -> bool {
        self.forced.is_none() && self.plan.is_none()
    }
}

/// The tone or code a channel carries.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ToneRead {
    /// "88.5 Hz", "D023N" or "D023I".
    pub label: String,
    /// The other reading of the same DCS word, when there is one.
    pub alias: String,
    /// The frequency of a CTCSS tone, Hz.
    pub hz: Option<f32>,
    /// The frequency the decoder measured, Hz.
    pub measured_hz: Option<f32>,
}

/// One 10 ms block of a channel.
#[derive(Clone, Debug)]
pub struct RxBlock {
    /// The squelch is open.
    pub open: bool,
    /// The power after the channel filter, dBFS.
    pub level_dbfs: f64,
    /// The FM squelch measure, dB.
    pub measure_db: f64,
    /// The demodulator in use.
    pub demod: Demod,
    /// The gated audio: zeros while the squelch is closed.
    pub audio: Vec<f32>,
    /// The voice band of the discriminator (300 to 3000 Hz, before de-emphasis), not gated. Empty without FM.
    pub flat: Vec<f32>,
}

/// One sideband demodulator: the band from 300 to 3000 Hz on one side of the channel frequency.
struct Side {
    rot: Complex32,
    step: Complex32,
    lp: [Butterworth4; 2],
    gain: f32,
    buf: Vec<f32>,
}

impl Side {
    /// `sign` -1 takes the band above the channel frequency, +1 the band below.
    fn new(sign: f64) -> Side {
        let w = sign * TAU * 1650.0 / AUDIO_RATE;
        Side {
            rot: Complex32::new(1.0, 0.0),
            step: Complex32::new(w.cos() as f32, w.sin() as f32),
            lp: [
                Butterworth4::lowpass(AUDIO_RATE, 1350.0),
                Butterworth4::lowpass(AUDIO_RATE, 1350.0),
            ],
            gain: 1.0,
            buf: Vec::with_capacity(BLOCK),
        }
    }
    fn push(&mut self, y: Complex32) {
        let z = y * self.rot;
        let z = Complex32::new(
            self.lp[0].process_sample(z.re),
            self.lp[1].process_sample(z.im),
        );
        self.buf.push((z * self.rot.conj()).re);
        self.rot *= self.step;
    }
    fn end_block(&mut self) {
        let n = self.rot.norm();
        if n > 0.0 {
            self.rot /= n;
        }
    }
}

/// The gain control of AM and SSB audio for one block: down at once when the peak would pass 0.5, else up by 0.3 % to `max`.
fn level_block(buf: &mut [f32], gain: &mut f32, max: f32) {
    let peak = buf.iter().fold(0.0f32, |m, v| m.max(v.abs()));
    if peak * *gain > 0.5 {
        *gain = 0.5 / peak;
    } else {
        *gain = (*gain * 1.003).min(max);
    }
    for v in buf {
        *v = (*v * *gain).clamp(-1.0, 1.0);
    }
}

/// The line statistics of the open blocks of a channel.
#[derive(Clone, Copy)]
struct Lines {
    /// The blocks counted, 100 at most.
    n: u32,
    mean: [f64; 51],
    square: [f64; 51],
    total: f64,
}

impl Lines {
    const NONE: Lines = Lines {
        n: 0,
        mean: [0.0; 51],
        square: [0.0; 51],
        total: 0.0,
    };
}

/// Which decoder locked last.
#[derive(Clone, Copy, PartialEq)]
enum Last {
    Ctcss,
    Dcs,
}

/// The voice channel receiver of one carrier.
pub struct VoiceRx {
    cfg: RxConfig,
    filter: [Butterworth4; 2],
    disc: Discriminator,
    hp: Biquad,
    lp: Biquad,
    hi: Biquad,
    de_y: f32,
    am_mean: f32,
    am_hp: Biquad,
    am_lp: Biquad,
    am_gain: f32,
    usb: Side,
    lsb: Side,
    fm: Vec<f32>,
    am: Vec<f32>,
    flat: Vec<f32>,
    raw: Vec<Complex32>,
    tone_in: Vec<f32>,
    // Sums of the block.
    pw: f64,
    l: f64,
    h: f64,
    mag: f64,
    mag2: f64,
    wf: f64,
    // The squelch.
    ls: f64,
    hs: f64,
    prev_db: f64,
    fm_run: u32,
    open: bool,
    opened_fm: bool,
    hang: u32,
    demod: Demod,
    since_open: u32,
    /// Blocks since the squelch closed.
    closed: u32,
    /// Blocks in a row with a signal far below the highest level.
    low: u32,
    votes: u32,
    variation: Option<f64>,
    lines: Lines,
    peak_db: f64,
    above: u32,
    below: u32,
    hist: VecDeque<RxBlock>,
    // The tone reader.
    ctcss: Option<CtcssDecoder>,
    dcs: Option<DcsDecoder>,
    ctcss_lock: Option<ToneRead>,
    dcs_lock: Option<ToneRead>,
    last: Last,
    events: Vec<SubAudioEvent>,
    window: Vec<f32>,
    turn: Vec<Complex32>,
}

impl VoiceRx {
    /// A receiver with the settings `cfg`.
    pub fn new(cfg: RxConfig) -> VoiceRx {
        let cut = cfg.cutoff_hz.max(3000.0);
        let voice =
            |f: fn(f64, f64, f64) -> BiquadCoeffs, hz: f64| Biquad::new(f(AUDIO_RATE, hz, 0.707));
        let reads = cfg.tones && cfg.uses(Demod::Fm);
        let n = BLOCK as f64;
        VoiceRx {
            cfg,
            filter: [
                Butterworth4::lowpass(AUDIO_RATE, cut),
                Butterworth4::lowpass(AUDIO_RATE, cut),
            ],
            disc: Discriminator::new(AUDIO_RATE, 5000.0)
                .expect("the rate and deviation are positive"),
            hp: voice(BiquadCoeffs::highpass, 300.0),
            lp: voice(BiquadCoeffs::lowpass, 3000.0),
            hi: voice(BiquadCoeffs::highpass, 5500.0),
            de_y: 0.0,
            am_mean: 0.0,
            am_hp: voice(BiquadCoeffs::highpass, 300.0),
            am_lp: voice(BiquadCoeffs::lowpass, 3000.0),
            am_gain: 1.0,
            usb: Side::new(-1.0),
            lsb: Side::new(1.0),
            fm: Vec::with_capacity(BLOCK),
            am: Vec::with_capacity(BLOCK),
            flat: Vec::with_capacity(BLOCK),
            raw: Vec::with_capacity(BLOCK),
            tone_in: Vec::with_capacity(BLOCK),
            pw: 0.0,
            l: 0.0,
            h: 0.0,
            mag: 0.0,
            mag2: 0.0,
            wf: 0.0,
            ls: 0.0,
            hs: 0.0,
            prev_db: -30.0,
            fm_run: 0,
            open: false,
            opened_fm: false,
            hang: 0,
            demod: cfg.forced.or(cfg.plan).unwrap_or(Demod::Am),
            since_open: 0,
            closed: u32::MAX,
            low: 0,
            votes: 0,
            variation: None,
            lines: Lines::NONE,
            peak_db: -200.0,
            above: 0,
            below: 0,
            hist: VecDeque::with_capacity(4),
            ctcss: reads
                .then(|| CtcssDecoder::new(AUDIO_RATE, CtcssConfig::default()).ok())
                .flatten(),
            dcs: reads
                .then(|| DcsDecoder::new(AUDIO_RATE, DcsConfig::default()).ok())
                .flatten(),
            ctcss_lock: None,
            dcs_lock: None,
            last: Last::Ctcss,
            events: Vec::new(),
            window: (0..BLOCK)
                .map(|i| (0.5 - 0.5 * (TAU * i as f64 / n).cos()) as f32)
                .collect(),
            turn: (0..BLOCK)
                .map(|i| {
                    let (s, c) = (-TAU * i as f64 / n).sin_cos();
                    Complex32::new(c as f32, s as f32)
                })
                .collect(),
        }
    }

    /// The settings.
    pub fn config(&self) -> &RxConfig {
        &self.cfg
    }

    /// Changes the squelch, the de-emphasis and the forced demodulator. The filters and the tone reader stay.
    pub fn set(&mut self, squelch: f64, deemphasis: bool, forced: Option<Demod>) {
        self.cfg.squelch = squelch;
        self.cfg.deemphasis = deemphasis;
        if self.cfg.forced != forced {
            self.cfg.forced = forced;
            if let Some(d) = forced.or(self.cfg.plan) {
                self.demod = d;
            }
        }
    }

    /// The tone or code the channel carries now.
    pub fn tone(&self) -> Option<&ToneRead> {
        match self.last {
            Last::Dcs => self.dcs_lock.as_ref().or(self.ctcss_lock.as_ref()),
            Last::Ctcss => self.ctcss_lock.as_ref().or(self.dcs_lock.as_ref()),
        }
    }

    /// The changes of the tone since the last call.
    pub fn take_tone_events(&mut self) -> Vec<SubAudioEvent> {
        std::mem::take(&mut self.events)
    }

    /// Takes the samples `x` of the channel at 48 kHz and appends a block to `out` for every 10 ms. `carrier` says that the
    /// carrier finder holds the carrier active now: it opens the squelch of AM and SSB.
    pub fn process(&mut self, x: &[Complex32], carrier: bool, out: &mut Vec<RxBlock>) {
        let (fm, am, usb, lsb) = (
            self.cfg.uses(Demod::Fm),
            self.cfg.uses(Demod::Am),
            self.cfg.uses(Demod::Usb),
            self.cfg.uses(Demod::Lsb),
        );
        let auto = self.cfg.auto();
        let de_a = (1.0 - (-1.0 / (750e-6 * AUDIO_RATE)).exp()) as f32;
        for &s in x {
            let y = Complex32::new(
                self.filter[0].process_sample(s.re),
                self.filter[1].process_sample(s.im),
            );
            let p = y.norm_sqr();
            self.pw += f64::from(p);
            if fm {
                let d = self.disc.process_sample(y);
                let vb = self.lp.process_sample(self.hp.process_sample(d));
                let hi = self.hi.process_sample(d);
                self.l += f64::from(vb * vb);
                self.h += f64::from(hi * hi);
                self.wf += f64::from(p * d);
                self.flat.push(vb);
                self.de_y += de_a * (vb - self.de_y);
                let a = if self.cfg.deemphasis {
                    self.de_y * 4.8
                } else {
                    vb
                };
                self.fm.push((a * 0.5).clamp(-1.0, 1.0));
                if self.ctcss.is_some() {
                    self.tone_in.push(d.clamp(-1.0, 1.0));
                }
            }
            if am || auto {
                let m = p.sqrt();
                self.mag += f64::from(m);
                self.mag2 += f64::from(p);
                if am {
                    self.am_mean += 0.0005 * (m - self.am_mean);
                    let e = if self.am_mean > 1e-9 {
                        m / self.am_mean - 1.0
                    } else {
                        0.0
                    };
                    let a = self.am_lp.process_sample(self.am_hp.process_sample(e));
                    self.am.push(a * 0.5);
                }
            }
            if usb {
                self.usb.push(y);
            }
            if lsb {
                self.lsb.push(y);
            }
            if auto {
                self.raw.push(y);
            }
            if (fm && self.fm.len() == BLOCK)
                || (am && self.am.len() == BLOCK)
                || (usb && self.usb.buf.len() == BLOCK)
                || (lsb && self.lsb.buf.len() == BLOCK)
            {
                self.end_block(carrier, out);
            }
        }
    }

    /// Adds the block to the line statistics: the power in each of 51 lines 100 Hz apart (Hann window; a carrier alone on
    /// a line has the whole power of the block) and the power of the block, as running means over the last second.
    fn add_lines(&mut self) {
        let (mut sw, mut sw2, mut pw) = (0.0f64, 0.0f64, 0.0f64);
        for (y, w) in self.raw.iter().zip(&self.window) {
            let w = f64::from(*w);
            sw += w;
            sw2 += w * w;
            pw += w * w * f64::from(y.norm_sqr());
        }
        if sw2 <= 0.0 {
            return;
        }
        self.lines.n = (self.lines.n + 1).min(100);
        let a = 1.0 / f64::from(self.lines.n);
        for (i, k) in (-25i32..=25).enumerate() {
            let mut acc = Complex32::default();
            let step = k.rem_euclid(BLOCK as i32) as usize;
            let mut at = 0usize;
            for (y, w) in self.raw.iter().zip(&self.window) {
                acc += *y * self.turn[at] * *w;
                at += step;
                if at >= BLOCK {
                    at -= BLOCK;
                }
            }
            let p = f64::from(acc.norm_sqr()) / (sw * sw);
            self.lines.mean[i] += a * (p - self.lines.mean[i]);
            self.lines.square[i] += a * (p * p - self.lines.square[i]);
        }
        self.lines.total += a * (pw / sw2 - self.lines.total);
    }

    /// Whether the signal has a carrier: over three blocks or more one line holds 0.45 of the power or more, and its power
    /// is steady from block to block. The strongest line of speech on one sideband moves and changes with the syllables.
    fn has_carrier(&self) -> bool {
        let l = &self.lines;
        let (k, top) =
            l.mean
                .iter()
                .enumerate()
                .fold((0, 0.0f64), |m, (i, p)| if *p > m.1 { (i, *p) } else { m });
        if l.n < 3 || l.total <= 0.0 || top < 0.45 * l.total {
            return false;
        }
        (l.square[k] - top * top).max(0.0).sqrt() / top <= 0.35
    }

    fn read_tones(&mut self) {
        let (Some(c), Some(d)) = (&mut self.ctcss, &mut self.dcs) else {
            return;
        };
        let events = &mut self.events;
        let (cl, dl, last) = (&mut self.ctcss_lock, &mut self.dcs_lock, &mut self.last);
        c.process(&self.tone_in, |e| {
            if e.kind == CtcssEventKind::Locked {
                *cl = Some(ToneRead {
                    label: tone_label(e.tone.hz),
                    alias: String::new(),
                    hz: Some(e.tone.hz),
                    measured_hz: Some(e.measured_hz),
                });
                *last = Last::Ctcss;
            } else {
                *cl = None;
            }
            events.push(SubAudioEvent::Ctcss(e));
        });
        d.process(&self.tone_in, |e| {
            if e.kind == DcsEventKind::Locked {
                *dl = Some(ToneRead {
                    label: e.reading.name(),
                    alias: e
                        .reading
                        .alias
                        .map(|(c, inv)| format!("{c}{}", if inv { 'I' } else { 'N' }))
                        .unwrap_or_default(),
                    hz: None,
                    measured_hz: None,
                });
                *last = Last::Dcs;
            } else {
                *dl = None;
            }
            events.push(SubAudioEvent::Dcs(e));
        });
        self.tone_in.clear();
        // A channel that is not heard as FM carries no tone.
        if self.demod != Demod::Fm {
            self.ctcss_lock = None;
            self.dcs_lock = None;
            self.events.clear();
        }
    }

    fn end_block(&mut self, carrier: bool, out: &mut Vec<RxBlock>) {
        let n = BLOCK as f64;
        let level = 10.0 * (self.pw / n).max(1e-30).log10();
        let fm = self.cfg.uses(Demod::Fm);
        let auto = self.cfg.auto();
        self.read_tones();
        // The FM measure: the voice band over the discriminator noise above 5.5 kHz.
        let measure = if !fm || level < -90.0 {
            self.prev_db = -30.0;
            -30.0
        } else {
            let (l, h) = (self.l / n, self.h / n);
            self.ls += 0.4 * (l - self.ls);
            self.hs += 0.4 * (h - self.hs);
            let smooth = 10.0 * (self.ls.max(1e-30) / self.hs.max(1e-30)).log10();
            let now = 10.0 * (l.max(1e-30) / h.max(1e-30)).log10();
            let fast = now.min(self.prev_db) - 3.0;
            self.prev_db = now;
            smooth.max(fast)
        };
        let fm_signal = fm && self.cfg.fm_squelch && measure > fm_threshold_db(self.cfg.squelch);
        let envelope_signal = carrier && (auto || self.demod != Demod::Fm);
        let mean = self.mag / n;
        let variation = if mean > 0.0 {
            (self.mag2 / n - mean * mean).max(0.0).sqrt() / mean
        } else {
            0.0
        };
        let v = self
            .variation
            .map_or(variation, |v| v + 0.1 * (variation - v));
        self.variation = Some(v);
        let mut signal = false;
        if self.open {
            signal = if self.opened_fm {
                fm_signal
            } else {
                fm_signal || envelope_signal
            };
            if signal {
                self.hang = if self.demod == Demod::Fm { 15 } else { 40 };
            } else {
                self.hang = self.hang.saturating_sub(1);
                if self.hang == 0 {
                    self.open = false;
                    self.fm_run = 0;
                    self.closed = 0;
                }
            }
        } else {
            self.closed = self.closed.saturating_add(1);
            self.fm_run = if fm_signal { self.fm_run + 1 } else { 0 };
            let by_fm = self.fm_run >= 2;
            if by_fm || envelope_signal {
                self.open = true;
                self.opened_fm = by_fm;
                self.votes = 0;
                // A channel that opens again within 2 s goes on with the same station: its demodulator and its line
                // statistics stay. Else the choice starts again: FM when the FM measure opened it, else AM.
                if self.closed > 200 {
                    self.since_open = 0;
                    self.lines = Lines::NONE;
                    self.peak_db = level;
                    self.above = 0;
                    self.below = 0;
                    if auto {
                        self.demod = Demod::Am;
                    }
                }
                if auto && by_fm {
                    self.demod = Demod::Fm;
                }
                self.hang = if self.demod == Demod::Fm { 15 } else { 40 };
            }
        }
        // A block votes while the signal is there and near its highest level: the hang and the pauses hold noise. A
        // block far above every block before it starts a new signal, and so does a signal that stays below that level
        // for 0.2 s (a weaker station answers).
        let weak = self.open && signal && level <= self.peak_db - 10.0;
        self.low = if weak { self.low + 1 } else { 0 };
        if level > self.peak_db + 10.0 || self.low >= 20 {
            self.lines = Lines::NONE;
            self.above = 0;
            self.below = 0;
            self.peak_db = level;
            self.low = 0;
        }
        self.peak_db = self.peak_db.max(level);
        if self.open && auto && signal && level > self.peak_db - 10.0 {
            self.add_lines();
            if self.wf > 0.0 {
                self.above += 1;
            } else if self.wf < 0.0 {
                self.below += 1;
            }
            let vote = if measure > 6.0 && v < 0.35 {
                Demod::Fm
            } else if self.has_carrier() {
                Demod::Am
            } else if self.above > self.below {
                Demod::Usb
            } else {
                Demod::Lsb
            };
            let sideband = |d: Demod| matches!(d, Demod::Usb | Demod::Lsb);
            if vote == self.demod {
                self.votes = 0;
            } else if sideband(vote) && sideband(self.demod) {
                self.demod = vote;
                self.votes = 0;
            } else {
                self.votes += 1;
                let need = if self.since_open <= 1 { 1 } else { 6 };
                if self.votes >= need {
                    self.demod = vote;
                    self.votes = 0;
                }
            }
            self.since_open += 1;
        }
        level_block(&mut self.am, &mut self.am_gain, 3.0);
        level_block(&mut self.usb.buf, &mut self.usb.gain, 4000.0);
        level_block(&mut self.lsb.buf, &mut self.lsb.gain, 4000.0);
        self.usb.end_block();
        self.lsb.end_block();
        let audio = match self.demod {
            Demod::Fm => &self.fm,
            Demod::Am => &self.am,
            Demod::Usb => &self.usb.buf,
            Demod::Lsb => &self.lsb.buf,
        };
        let mut audio = audio.clone();
        audio.resize(BLOCK, 0.0);
        let b = RxBlock {
            open: self.open,
            level_dbfs: level,
            measure_db: measure,
            demod: self.demod,
            audio,
            flat: self.flat.clone(),
        };
        self.fm.clear();
        self.am.clear();
        self.usb.buf.clear();
        self.lsb.buf.clear();
        self.flat.clear();
        self.raw.clear();
        self.pw = 0.0;
        self.l = 0.0;
        self.h = 0.0;
        self.mag = 0.0;
        self.mag2 = 0.0;
        self.wf = 0.0;
        // The audio runs 3 blocks late: a block is let through when it, or one of the 3 after it, is open.
        self.hist.push_back(b);
        if self.hist.len() > 3 {
            let gate = self.hist.iter().any(|h| h.open);
            if let Some(mut first) = self.hist.pop_front() {
                if !gate {
                    first.audio.iter_mut().for_each(|v| *v = 0.0);
                }
                first.open = first.open || (gate && self.hist.back().is_some_and(|h| h.open));
                out.push(first);
            }
        }
    }
}
