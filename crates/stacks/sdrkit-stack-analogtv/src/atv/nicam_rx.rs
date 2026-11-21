//! The NICAM-728 decoder: it takes the DQPSK carrier above the vision carrier and returns 48 kHz stereo. The matched filter and the
//! symbol timing are `sdrkit_algo_modem::timing::SymbolSync` with a Gardner detector.
//!
//! Standard: ETSI EN 300 163.

use super::{last, NicamReport, Tune};
use crate::nicam::{
    decode_frame, scrambling_bits, Decoded, Mode, FAW, FRAME_BITS, SYMBOL_RATE as NICAM_SYMBOL_RATE,
};
use crate::settings::{DualSound, DualSoundSetting, DUAL_SOUND};
use sdrkit_algo_dsp::decim::FirDecimator;
use sdrkit_algo_dsp::iir::Deemphasis;
use sdrkit_algo_dsp::nco::Nco;
use sdrkit_algo_dsp::pulse::Shape;
use sdrkit_algo_dsp::resample::Resampler;
use sdrkit_algo_dsp::Frame;
use sdrkit_algo_modem::timing::{Gardner, MatchedInterp, SymbolSync, SymbolSyncConfig};
use sdrkit_core::{
    Choice, Complex32, ConfigError, SettingError, SettingId, SettingSet, Value, Values,
};
use sdrkit_graph::{
    values_of, Applied, ChannelsFrom, Component, ComponentInfo, Format, Io, Kind, Port, RateFrom,
    Timed,
};
use std::collections::VecDeque;
use std::f64::consts::TAU;

/// Samples a symbol after resampling.
const SPS: f64 = 4.0;

/// The matched filter and interpolator: 32 taps (8 symbols) and 32 phases.
type Matched = MatchedInterp<32, 32>;

/// The symbol timing loop of the decoder.
type Timing = SymbolSync<Complex32, Gardner, Matched>;

/// The timing loop for the roll-off `alpha` (1.0 in system I, 0.4 elsewhere, EN 300 163 clause 3.8).
fn timing(alpha: f64) -> Option<Timing> {
    let mut cfg = SymbolSyncConfig::new(SPS);
    cfg.pulse = Shape::Rc { beta: alpha };
    cfg.bn_t = 0.01;
    let m = Matched::new(Shape::Rrc { beta: alpha }, SPS).ok()?;
    SymbolSync::new(Gardner, m, cfg).ok()
}

/// The NICAM decoder.
pub struct NicamDecoder {
    fs: f64,
    mix: Nco,
    dec: FirDecimator<Complex32>,
    res: Option<Resampler<Complex32>>,
    sync: Option<Timing>,
    alpha: f64,
    a: Vec<Complex32>,
    b: Vec<Complex32>,
    c: Vec<Complex32>,
    syms: Vec<Complex32>,
    last_sym: Complex32,
    rot: Complex32,
    bits: VecDeque<u8>,
    bit_index: u64,
    locked: bool,
    lock_phase: u64,
    misses: u32,
    rejected: VecDeque<u64>,
    c0_run: (u8, u32),
    data_run: u32,
    frames: u64,
    frame_idx: Option<u64>,
    last_c0: u8,
    prev: [i32; 2],
    dec_frame: Decoded,
    deemph: [Deemphasis; 2],
    out32: Vec<Frame<2>>,
    out48: Vec<Frame<2>>,
    res48: Resampler<Frame<2>>,
    mode: Mode,
    c4: bool,
    parity_errors: u64,
    samples: u64,
    error_rate: f64,
    level: f64,
    tune: Tune,
    epoch: u32,
    active: bool,
    carrier: f64,
    /// Two languages: play the second.
    pub second: bool,
    log: Vec<String>,
    mode_seen: Option<Mode>,
}

impl Default for NicamDecoder {
    fn default() -> Self {
        NicamDecoder {
            fs: 1.0,
            mix: Nco::new(1.0, 0.0),
            dec: FirDecimator::new(&[1.0], 1),
            res: None,
            sync: None,
            alpha: f64::NAN,
            a: Vec::new(),
            b: Vec::new(),
            c: Vec::new(),
            syms: Vec::new(),
            last_sym: Complex32::default(),
            rot: Complex32::new(1.0, 0.0),
            bits: VecDeque::with_capacity(4 * FRAME_BITS),
            bit_index: 0,
            locked: false,
            lock_phase: 0,
            misses: 0,
            rejected: VecDeque::new(),
            c0_run: (2, 0),
            data_run: 0,
            frames: 0,
            frame_idx: None,
            last_c0: 2,
            prev: [0; 2],
            dec_frame: Decoded::default(),
            deemph: [crate::nicam::j17_de(), crate::nicam::j17_de()],
            out32: Vec::new(),
            out48: Vec::new(),
            res48: Resampler::new(32_000.0, 48_000.0).expect("32 to 48 kHz"),
            mode: Mode::Data,
            c4: false,
            parity_errors: 0,
            samples: 0,
            error_rate: 1.0,
            level: -100.0,
            tune: Tune::default(),
            epoch: u32::MAX,
            active: false,
            carrier: f64::NAN,
            second: false,
            log: Vec::new(),
            mode_seen: None,
        }
    }
}

impl NicamDecoder {
    fn reset(&mut self) {
        self.dec.reset();
        if let Some(r) = &mut self.res {
            r.reset();
        }
        if let Some(s) = &mut self.sync {
            s.reset();
        }
        self.bits.clear();
        self.bit_index = 0;
        self.unlock();
        self.rejected.clear();
        self.frames = 0;
        self.parity_errors = 0;
        self.samples = 0;
        self.error_rate = 1.0;
        self.rot = Complex32::new(1.0, 0.0);
        self.mode_seen = None;
    }

    fn unlock(&mut self) {
        self.locked = false;
        self.misses = 0;
        self.c0_run = (2, 0);
        self.data_run = 0;
        self.frame_idx = None;
        self.frames = 0;
    }

    fn bit_at(&self, i: u64) -> Option<u8> {
        let first = self.bit_index - self.bits.len() as u64;
        if i < first || i >= self.bit_index {
            None
        } else {
            Some(self.bits[(i - first) as usize])
        }
    }

    fn faw_errors(&self, end: u64) -> u32 {
        // The FAW occupies bits [end - 8, end).
        let mut e = 0;
        for (j, &f) in FAW.iter().enumerate() {
            match self.bit_at(end - 8 + j as u64) {
                Some(b) if b == f => {}
                _ => e += 1,
            }
        }
        e
    }

    fn decode_at(&mut self, faw_end: u64) {
        let mut bits = [0u8; 720];
        for (j, b) in bits.iter_mut().enumerate() {
            *b = self.bit_at(faw_end + j as u64).unwrap_or(0);
        }
        let mut d = std::mem::take(&mut self.dec_frame);
        decode_frame(&bits, scrambling_bits(), &mut self.prev, &mut d);
        let c = d.control;
        // The frame flag sequence: eight ones then eight zeros.
        if c.c0 == self.c0_run.0 {
            self.c0_run.1 += 1;
        } else {
            if self.last_c0 == 0 && c.c0 == 1 {
                self.frame_idx = Some(0);
            }
            self.c0_run = (c.c0, 1);
        }
        self.last_c0 = c.c0;
        let mode = Mode::of(c.mode);
        if mode == Mode::Data {
            self.data_run += 1;
        } else {
            self.data_run = 0;
        }
        if self.c0_run.1 > 9 || self.data_run >= 8 {
            self.reject();
            self.dec_frame = d;
            return;
        }
        self.mode = mode;
        self.c4 = c.c4 == 1;
        if self.mode_seen != Some(mode) {
            self.mode_seen = Some(mode);
            self.log.push(format!("NICAM: {}", mode.label()));
        }
        let bad = d.bad.count_ones() as u64;
        self.parity_errors += bad;
        self.samples += 64;
        self.error_rate += 0.02 * (bad as f64 / 64.0 - self.error_rate);
        self.frames += 1;
        let idx = self.frame_idx.map(|i| i + 1);
        let parity = self.frame_idx.unwrap_or(0) % 2;
        self.frame_idx = idx.or(self.frame_idx);
        let scale = 1.0 / 8192.0;
        match mode {
            Mode::Stereo => {
                for k in 0..32 {
                    let l = self.deemph[0].process_f64(d.samples[2 * k] as f64 * scale) as f32;
                    let r = self.deemph[1].process_f64(d.samples[2 * k + 1] as f64 * scale) as f32;
                    self.out32.push(Frame([l, r]));
                }
            }
            Mode::DualMono | Mode::MonoData => {
                // M1 in odd frames, M2 in even frames.
                let want = if self.second && mode == Mode::DualMono {
                    0
                } else {
                    1
                };
                if parity == want {
                    for k in 0..64 {
                        let v = self.deemph[0].process_f64(d.samples[k] as f64 * scale) as f32;
                        self.out32.push(Frame([v, v]));
                    }
                }
            }
            Mode::Data => {}
        }
        self.dec_frame = d;
    }

    fn reject(&mut self) {
        if self.rejected.len() >= 12 {
            self.rejected.pop_front();
        }
        self.rejected.push_back(self.lock_phase);
        self.unlock();
    }

    fn on_bit(&mut self, b: u8) {
        if self.bits.len() >= 4 * FRAME_BITS {
            self.bits.pop_front();
        }
        self.bits.push_back(b);
        self.bit_index += 1;
        let i = self.bit_index;
        let fl = FRAME_BITS as u64;
        if self.locked {
            // A frame is complete when its 720 bits after the FAW are in: check the next FAW then.
            if i >= self.lock_phase + fl && (i - self.lock_phase).is_multiple_of(fl) {
                let faw_end = i - fl;
                // The frame starting at faw_end is complete at faw_end + 720; the next FAW ends at faw_end + 728 = i.
                if self.faw_errors(i) <= 1 {
                    self.misses = 0;
                } else {
                    self.misses += 1;
                    if self.misses >= 4 {
                        self.log.push("NICAM: frame alignment lost".into());
                        self.unlock();
                        return;
                    }
                }
                self.decode_at(faw_end);
            }
        } else if i >= 2 * fl + 8
            && self.faw_errors(i) == 0
            && self.faw_errors(i - fl) == 0
            && self.faw_errors(i - 2 * fl) == 0
        {
            let phase = i % fl;
            if self.rejected.contains(&phase) {
                return;
            }
            self.locked = true;
            self.error_rate = 0.0;
            // The counts are those of this lock.
            self.parity_errors = 0;
            self.samples = 0;
            self.lock_phase = i % fl;
            self.misses = 0;
            self.log.push("NICAM: frame alignment found".into());
            // The two frames already buffered.
            self.decode_at(i - 2 * fl);
            self.decode_at(i - fl);
        }
    }

    fn symbol(&mut self, s: Complex32) {
        let d = s * self.last_sym.conj();
        self.last_sym = s;
        let m = d.norm();
        if m <= 0.0 {
            return;
        }
        let u = d / m;
        let u4 = u * u * u * u;
        self.rot = self.rot * 0.995 + u4 * 0.005;
        let corr = Complex32::from_polar(1.0, -self.rot.arg() / 4.0);
        // The d^4 of a QPSK step is -1 at 45° offset; DQPSK steps sit on the axes (d^4 = 1).
        let z = u * corr;
        let (b0, b1) = if z.re.abs() >= z.im.abs() {
            if z.re >= 0.0 {
                (0, 0)
            } else {
                (1, 1)
            }
        } else if z.im < 0.0 {
            (0, 1)
        } else {
            (1, 0)
        };
        self.on_bit(b0);
        self.on_bit(b1);
    }
}

impl Component for NicamDecoder {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "atv.nicam",
            vec![Port::iq("in"), Port::typed::<Tune>("control")],
            vec![
                Port::audio("out")
                    .rate_from(RateFrom::Fixed(48_000.0))
                    .channels_from(ChannelsFrom::Fixed(2)),
                Port::typed::<NicamReport>("report").rate_from(RateFrom::Fixed(0.0)),
            ],
        )
        .with_settings(DualSoundSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.fs = inputs[0].rate;
        let d = (self.fs / 1.25e6).round().max(1.0) as usize;
        let fs1 = self.fs / d as f64;
        self.mix = Nco::new(self.fs, 0.0);
        self.dec = if d > 1 {
            FirDecimator::design(
                self.fs,
                d,
                400e3,
                (fs1 - 600e3).max(420e3).min(fs1 - 400e3),
                50.0,
            )?
        } else {
            FirDecimator::new(&[1.0], 1)
        };
        let fs4 = NICAM_SYMBOL_RATE * SPS;
        self.res = Resampler::with_quality(fs1, fs4, 0.45, 50.0).ok();
        self.epoch = u32::MAX;
        self.active = false;
        self.carrier = f64::NAN;
        self.reset();
        Ok(vec![
            Format::audio(48_000.0, 2),
            Format::new(Kind::typed::<NicamReport>(), 0.0),
        ])
    }
    fn process(&mut self, io: &mut Io) {
        // The output states its two channels in every pass, also one without sound.
        io.audio_out(0);
        if let Some(t) = last::<Tune>(io, 1) {
            if t.epoch != self.epoch {
                self.epoch = t.epoch;
                self.reset();
            }
            let sys_i = (t.spacing_hz.abs() - 6.0e6).abs() < 0.2e6;
            let off = if sys_i { 6.552e6 } else { 5.85e6 };
            let alpha = if sys_i { 1.0 } else { 0.4 };
            let car = t.carrier_hz + off;
            if alpha != self.alpha {
                self.alpha = alpha;
                self.sync = timing(alpha);
            }
            if car != self.carrier {
                self.carrier = car;
                self.mix.set_freq(-car);
            }
            let was = self.active;
            self.active = t.second_sound && t.spacing_hz != 0.0;
            if was && !self.active {
                self.reset();
            }
            self.tune = t;
        }
        let x = io.iq(0);
        self.out32.clear();
        if self.active {
            self.a.clear();
            self.a.extend_from_slice(x);
            self.mix.mix_in_place(&mut self.a);
            self.b.clear();
            self.dec.process(&self.a, &mut self.b);
            let mut p = 0.0f64;
            for s in &self.b {
                p += s.norm_sqr() as f64;
            }
            if !self.b.is_empty() {
                let lv = 10.0 * (p / self.b.len() as f64).max(1e-20).log10();
                self.level += 0.1 * (lv - self.level);
            }
            self.c.clear();
            if let Some(r) = &mut self.res {
                r.process(&self.b, &mut self.c);
            }
            self.syms.clear();
            if let Some(sync) = &mut self.sync {
                sync.process(&self.c, &mut self.syms);
            }
            for k in 0..self.syms.len() {
                self.symbol(self.syms[k]);
            }
        }
        let valid =
            self.locked && self.frames > 40 && self.error_rate < 0.15 && self.mode != Mode::Data;
        if valid || self.locked {
            self.out48.clear();
            self.res48.process(&self.out32, &mut self.out48);
            io.audio_out(0)
                .samples
                .extend_from_slice(Frame::flat(&self.out48));
        }
        if io.tick() {
            let t = io.input_format(0).time_of(io.input(0).first());
            io.items_out::<NicamReport>(1).push(Timed {
                time: t,
                value: NicamReport {
                    locked: self.locked,
                    valid,
                    mode: self.locked.then_some(self.mode),
                    c4: self.c4,
                    frames: self.frames,
                    parity_errors: self.parity_errors,
                    samples: self.samples,
                    error_rate: self.error_rate,
                    level_dbfs: self.level,
                    carrier_error_hz: (self.rot.arg() as f64 / 4.0) * NICAM_SYMBOL_RATE / TAU,
                    log: std::mem::take(&mut self.log),
                },
            });
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match DualSoundSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            DualSoundSetting::DualSound => {
                self.second = DUAL_SOUND.read(id, v)? == DualSound::Second;
            }
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        let d = if self.second {
            DualSound::Second
        } else {
            DualSound::First
        };
        values_of([(DualSoundSetting::DualSound, Value::Choice(d.index()))])
    }
}
