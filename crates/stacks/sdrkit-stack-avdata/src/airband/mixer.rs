//! The audio mixer: every open, unmuted channel (only the solo ones when any is solo), a priority channel above the others, or
//! scan (one channel at a time), a soft limit, and the optional noise remover.

use super::{ChannelAudio, Gate, ScanState, LISTEN, SCAN, SCAN_HOLD};
use sdrkit_algo_dsp::fft::Fft;
use sdrkit_core::publish::field;
use sdrkit_core::{
    Complex32, ConfigError, Setting, SettingError, SettingId, SettingSet, Value, Values,
};
use sdrkit_graph::{
    values_of, Applied, Component, ComponentInfo, Format, Io, Kind, Port, RateFrom, SlicePool,
};
use std::sync::Arc;

sdrkit_core::settings! {
    /// The settings of [`Mixer`].
    pub enum MixerSetting {
        /// Scan the open channels.
        Scan = SCAN,
        /// Stay on the channel the scan stopped on.
        ScanHold = SCAN_HOLD,
        /// Play the audio.
        Listen = LISTEN,
    }
}

/// The noise remover is on.
pub const ON: Setting<field::Bool> = Setting::new("on", "On", "", false);
/// How much of the noise remover's output is heard.
pub const STRENGTH: Setting<field::F64> = Setting::new("strength", "Strength", "%", 80.0)
    .range(0.0, 100.0, 1.0)
    .help("How much of the noise remover's output is heard.");

sdrkit_core::settings! {
    /// The settings of [`NoiseRemover`].
    pub enum NoiseRemoverSetting {
        /// On.
        On = ON,
        /// Strength.
        Strength = STRENGTH,
    }
}

/// The mixer.
pub struct Mixer {
    scan: bool,
    hold: bool,
    listen: bool,
    /// The channel the scan stopped on (list index), or -1 while scanning.
    pub scan_channel: i64,
    closed_at: f64,
    t: f64,
    reported: ScanState,
    g: Vec<f32>,
    playing_pool: SlicePool<i64>,
}

impl Default for Mixer {
    fn default() -> Self {
        Mixer {
            scan: SCAN.default,
            hold: SCAN_HOLD.default,
            listen: LISTEN.default,
            scan_channel: -1,
            closed_at: f64::NEG_INFINITY,
            t: 0.0,
            reported: ScanState::default(),
            g: Vec::new(),
            playing_pool: SlicePool::default(),
        }
    }
}

impl Mixer {
    /// Writes into `g` the gains of the channels for their gates now, and moves the scan channel.
    pub fn gains(&mut self, st: &[Gate], g: &mut Vec<f32>) {
        let n = st.len();
        g.clear();
        g.resize(n, 0.0);
        if self.scan {
            let open = |i: usize| st[i].open && !st[i].mute;
            let prio = st.iter().position(|s| s.priority);
            if let Some(p) = prio {
                if open(p) && self.scan_channel != p as i64 {
                    self.scan_channel = p as i64;
                    self.closed_at = self.t;
                }
            }
            if self.scan_channel >= 0 {
                let c = self.scan_channel as usize;
                if c < n && open(c) {
                    self.closed_at = self.t;
                } else if !self.hold && self.t - self.closed_at > 2.0 {
                    self.scan_channel = -1;
                }
            }
            if self.scan_channel < 0 {
                if let Some(i) = prio.into_iter().chain(0..n).find(|&i| open(i)) {
                    self.scan_channel = i as i64;
                    self.closed_at = self.t;
                }
            }
            if self.scan_channel >= 0 {
                let c = self.scan_channel as usize;
                if c < n && open(c) {
                    g[c] = 1.0;
                }
            }
            return;
        }
        let any_solo = st.iter().any(|s| s.solo);
        let prio_open = st.iter().any(|s| s.priority && s.open && !s.mute);
        for (i, s) in st.iter().enumerate() {
            if !s.open || s.mute || (any_solo && !s.solo) {
                continue;
            }
            g[i] = if prio_open && !s.priority { 0.25 } else { 1.0 };
        }
    }
}

impl Component for Mixer {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "avdata.airband_mixer",
            vec![Port::typed::<ChannelAudio>("in")],
            vec![
                Port::real("out").rate_from(RateFrom::Fixed(8000.0)),
                Port::typed::<ScanState>("scan").rate_from(RateFrom::Fixed(0.0)),
            ],
        )
        .with_settings(MixerSetting::schema())
    }
    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.t = 0.0;
        self.scan_channel = -1;
        let mut f = Format::new(Kind::Real, 8000.0);
        f.bandwidth_hz = 3600.0;
        Ok(vec![f, Format::new(Kind::typed::<ScanState>(), 0.0)])
    }
    fn process(&mut self, io: &mut Io) {
        let items = io.items::<ChannelAudio>(0);
        let mut g = std::mem::take(&mut self.g);
        let mut last: Option<Arc<[Gate]>> = None;
        let out = io.real_out(0);
        for it in items {
            let a = &it.value;
            self.gains(&a.gates, &mut g);
            for k in 0..a.len {
                let mut s = 0.0f32;
                for (c, gc) in g.iter().enumerate() {
                    if *gc > 0.0 {
                        s += a.channel(c)[k] * gc;
                    }
                }
                out.push(if self.listen { s.tanh() } else { 0.0 });
            }
            self.t += a.len as f64 / 8000.0;
            last = Some(Arc::clone(&a.gates));
        }
        if let Some(gates) = last {
            let channel = if self.scan && self.scan_channel >= 0 {
                gates
                    .get(self.scan_channel as usize)
                    .map_or(-1, |s| s.freq_hz as i64)
            } else {
                -1
            };
            let playing = gates
                .iter()
                .zip(&g)
                .filter(|(_, g)| **g > 0.0)
                .map(|(s, _)| s.freq_hz as i64);
            let same = self.reported.scan == self.scan
                && self.reported.channel == channel
                && self.reported.playing.iter().copied().eq(playing.clone());
            if io.tick() || !same {
                let n = playing.clone().count();
                self.reported = ScanState {
                    scan: self.scan,
                    channel,
                    playing: self.playing_pool.exact(n, |o| {
                        for (d, f) in o.iter_mut().zip(playing) {
                            *d = f;
                        }
                    }),
                };
                let t = self.t;
                io.items_out::<ScanState>(1).push(sdrkit_graph::Timed {
                    time: t,
                    value: self.reported.clone(),
                });
            }
        }
        self.g = g;
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match MixerSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            MixerSetting::Scan => {
                self.scan = SCAN.read(id, v)?;
                self.scan_channel = -1;
            }
            MixerSetting::ScanHold => self.hold = SCAN_HOLD.read(id, v)?,
            MixerSetting::Listen => self.listen = LISTEN.read(id, v)?,
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([
            (MixerSetting::Scan, Value::Bool(self.scan)),
            (MixerSetting::ScanHold, Value::Bool(self.hold)),
            (MixerSetting::Listen, Value::Bool(self.listen)),
        ])
    }
}

/// A speech noise remover: a short-time spectral gain from a minimum-statistics noise estimate (256-point frames, half overlap, about
/// 32 ms of delay), mixed with the unprocessed audio by the strength. A permissively licensed neural denoiser can take its place.
pub struct NoiseRemover {
    on: bool,
    strength: f32,
    fft: Fft,
    ifft: Fft,
    win: Vec<f32>,
    inbuf: Vec<f32>,
    outbuf: Vec<f32>,
    noise: Vec<f32>,
    frames: u64,
    queue: std::collections::VecDeque<f32>,
}

const NR_N: usize = 256;

impl Default for NoiseRemover {
    fn default() -> Self {
        NoiseRemover {
            on: ON.default,
            strength: (STRENGTH.default / 100.0) as f32,
            fft: Fft::forward(NR_N),
            ifft: Fft::inverse(NR_N),
            win: (0..NR_N)
                .map(|i| (std::f64::consts::PI * (i as f64 + 0.5) / NR_N as f64).sin() as f32)
                .collect(),
            inbuf: Vec::new(),
            outbuf: vec![0.0; NR_N],
            noise: vec![0.0; NR_N],
            frames: 0,
            queue: Default::default(),
        }
    }
}

impl NoiseRemover {
    fn frame(&mut self) {
        let mut b: Vec<Complex32> = self.inbuf[..NR_N]
            .iter()
            .zip(&self.win)
            .map(|(x, w)| Complex32::new(x * w, 0.0))
            .collect();
        self.fft.process(&mut b);
        for (k, v) in b.iter_mut().enumerate() {
            let p = v.norm_sqr();
            let nz = &mut self.noise[k];
            if self.frames == 0 || p < *nz {
                *nz = p;
            } else {
                *nz *= 1.002;
            }
            let g = (1.0 - 2.0 * *nz / p.max(1e-20)).clamp(0.05, 1.0);
            *v *= g * self.strength + (1.0 - self.strength);
        }
        self.frames += 1;
        self.ifft.process(&mut b);
        let half = NR_N / 2;
        for ((o, v), w) in self
            .outbuf
            .iter_mut()
            .zip(b.iter())
            .zip(self.win.iter())
            .take(NR_N)
        {
            *o += v.re / NR_N as f32 * w;
        }
        for k in 0..half {
            self.queue.push_back(self.outbuf[k]);
        }
        self.outbuf.drain(..half);
        self.outbuf.extend(std::iter::repeat_n(0.0, half));
        self.inbuf.drain(..half);
    }
}

impl Component for NoiseRemover {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "avdata.airband_noise_remover",
            vec![Port::real("in")],
            vec![Port::real("out").same_format_as(0)],
        )
        .with_settings(NoiseRemoverSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        Ok(vec![inputs[0].clone()])
    }
    fn process(&mut self, io: &mut Io) {
        let x = io.real(0);
        if !self.on {
            io.real_out(0).extend_from_slice(x);
            return;
        }
        let out = io.real_out(0);
        for &v in x {
            self.inbuf.push(v);
            if self.inbuf.len() >= NR_N {
                self.frame();
            }
            out.push(self.queue.pop_front().unwrap_or(0.0));
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match NoiseRemoverSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            NoiseRemoverSetting::On => self.on = ON.read(id, v)?,
            NoiseRemoverSetting::Strength => {
                self.strength = (STRENGTH.read(id, v)? / 100.0) as f32;
            }
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([
            (NoiseRemoverSetting::On, Value::Bool(self.on)),
            (
                NoiseRemoverSetting::Strength,
                Value::F64(f64::from(self.strength) * 100.0),
            ),
        ])
    }
}
