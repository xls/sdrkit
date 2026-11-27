//! The `fpv` receiver: analogue FPV video on the 5.8 GHz and 1.2 GHz channels. A channel meter watches the whole capture,
//! the channel of the setting `channels` nearest the centre is mixed to 0 Hz, the `atv` FM-video chain makes the picture,
//! and a decoder plays the sound subcarrier.

use crate::atv::fmvideo::FmVideoDisc;
use crate::atv::picture::PictureDecoder;
use crate::atv::sound::AUDIO_HZ;
use crate::atv::supervisor::Supervisor;
use crate::atv::sync::VideoSync;
use crate::atv::{last, Modulation, PictureReport, SyncReport, TICK_S};
use crate::channels::{
    fpv_channels, fpv_names_at, fpv_nearest, fpv_scan_centres, FpvChannel, CHANNELS,
    FPV_CHANNELS_CAP,
};
use crate::results::{fpv_published, publish_fpv, series_id, write_series, FmPolarity, State};
use crate::settings::{
    FpvSystem, SoundSubcarrier, AUDIO, COLOUR_ON, DEINTERLACE, FPV_SYSTEM, SATURATION,
    SOUND_SUBCARRIER,
};
use crate::typed::{
    FpvChannelArgs, FpvScanArgs, FpvStateEvent, FPV_CHANNELS, FPV_CHANNEL_TUNE, FPV_SCAN, FPV_STATE,
};
use sdrkit_algo_dsp::decim::Decimator;
use sdrkit_algo_dsp::fft::Fft;
use sdrkit_algo_dsp::meter::power_to_db as db10;
use sdrkit_algo_dsp::nco::Nco;
use sdrkit_algo_dsp::resample::{FractionalResampler, Resampler};
use sdrkit_algo_dsp::window::Window;
use sdrkit_core::command::{Command, CommandSet};
use sdrkit_core::publish::keys::Family;
use sdrkit_core::publish::Published;
use sdrkit_core::{
    Choice, CommandError, CommandId, Commands, Complex32, ConfigError, Generator, HostRequest, Hz,
    Record, Results, SeriesId, SettingError, SettingId, SettingSet, StackConfig, Status, Value,
    Values,
};
use sdrkit_graph::{
    values_of, Applied, Catalog, Component, ComponentInfo, Format, Graph, GraphBuilder, GraphError,
    Io, Item, Kept, Kind, Port, RateFrom, StackGraph, TagKind, Timed,
};
use std::f64::consts::TAU;
use std::sync::Arc;

sdrkit_core::settings! {
    /// The setting of the FPV components that hold the channels.
    pub enum FpvChannelsSetting {
        /// The channels.
        Channels = CHANNELS,
    }
}

/// The channels of the setting [`CHANNELS`] that an FPV component holds.
#[derive(Clone, Debug, Default)]
pub struct ChannelList {
    text: String,
    list: Vec<FpvChannel>,
}

impl ChannelList {
    /// Applies a value of [`CHANNELS`]. True when the list changed.
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<bool, SettingError> {
        match FpvChannelsSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            FpvChannelsSetting::Channels => {
                let text = CHANNELS.read(id, v)?;
                let list = fpv_channels(&crate::channels::read(id, text, FPV_CHANNELS_CAP)?);
                text.clone_into(&mut self.text);
                let changed = list != self.list;
                self.list = list;
                Ok(changed)
            }
        }
    }
    fn values(&self) -> Values {
        values_of([(FpvChannelsSetting::Channels, Value::Text(self.text.clone()))])
    }
}

/// One row of the channel table.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ChannelRow {
    /// "R5".
    pub name: String,
    /// Band letter.
    pub band: String,
    /// Number.
    pub number: u8,
    /// MHz.
    pub mhz: f64,
    /// Level, dBFS.
    pub level_dbfs: f64,
    /// Over the noise, dB.
    pub over_db: f64,
    /// A transmitter is there (8 dB over the noise).
    pub present: bool,
    /// Inside the capture now.
    pub inside: bool,
    /// The stream time of the last measurement, in seconds (none: never).
    pub heard_s: Option<f64>,
}

/// The channel meter's report.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MeterReport {
    /// The channel table, one row per channel.
    pub table: Vec<ChannelRow>,
    /// The tuned channel's name ("" off-plan).
    pub tuned: String,
    /// The tuned frequency, MHz.
    pub tuned_mhz: f64,
    /// Level of the tuned channel, dBFS.
    pub level_dbfs: f64,
    /// Over the noise, dB.
    pub over_db: f64,
    /// The noise floor in the channel window, dBFS.
    pub floor_dbfs: f64,
    /// Carrier offset of the tuned channel, Hz.
    pub offset_hz: f64,
    /// Display spectrum (MHz, dB).
    pub spectrum: Vec<[f32; 2]>,
    /// Level history (one point a second).
    pub history: Vec<[f32; 2]>,
}
impl Item for MeterReport {
    const KIND: &'static str = "fpv.meter_report";
}

const NFFT: usize = 1024;

/// The channel meter.
pub struct ChannelMeter {
    fs: f64,
    centre_mhz: f64,
    fft: Fft,
    win: Vec<f32>,
    /// `NFFT` times the sum of the squared window: the bin powers over it sum to the power of the input in dBFS terms.
    norm: f64,
    buf: Vec<Complex32>,
    work: Vec<Complex32>,
    acc: Vec<f64>,
    n_acc: u32,
    every: u64,
    pos: u64,
    floor: Vec<f64>,
    floor_init: bool,
    channels: ChannelList,
    rows: Vec<ChannelRow>,
    report: MeterReport,
    hist_t: f64,
    ticks: u32,
}

/// The rows of the channel table, one for each of `channels`, not heard yet.
fn channel_rows(channels: &[FpvChannel]) -> Vec<ChannelRow> {
    channels
        .iter()
        .map(|c| ChannelRow {
            name: c.name.clone(),
            band: c.band.clone(),
            number: c.number,
            mhz: c.mhz,
            heard_s: None,
            level_dbfs: -200.0,
            ..Default::default()
        })
        .collect()
}

impl Default for ChannelMeter {
    fn default() -> Self {
        ChannelMeter {
            fs: 20e6,
            centre_mhz: 0.0,
            fft: Fft::forward(NFFT),
            win: Window::Hann.coefficients(NFFT),
            norm: NFFT as f64
                * Window::Hann
                    .periodic(NFFT)
                    .iter()
                    .map(|w| w * w)
                    .sum::<f64>(),
            buf: Vec::with_capacity(NFFT),
            work: vec![Complex32::default(); NFFT],
            acc: vec![0.0; NFFT],
            n_acc: 0,
            every: 50_000,
            pos: 0,
            floor: vec![0.0; NFFT],
            channels: ChannelList::default(),
            floor_init: false,
            rows: Vec::new(),
            report: MeterReport::default(),
            hist_t: 0.0,
            ticks: 0,
        }
    }
}

impl ChannelMeter {
    /// Forgets the levels, the noise floor and the history (the Clear command).
    pub fn clear(&mut self) {
        for r in &mut self.rows {
            r.level_dbfs = -200.0;
            r.over_db = 0.0;
            r.present = false;
            r.heard_s = None;
        }
        self.floor_init = false;
        self.report.history.clear();
    }

    fn judge(&mut self, dt: f64, now_s: f64) {
        let n = NFFT;
        // The power of each bin, dBFS terms: a full-scale carrier sums to 1 over its bins.
        let scale = 1.0 / (self.n_acc.max(1) as f64 * self.norm);
        let p: Vec<f64> = (0..n).map(|k| self.acc[(k + n / 2) % n] * scale).collect();
        // The noise floor: 15th percentile of the central 80 % of bins x 1.15; falls at once, rises at most 0.2 dB a second.
        let mut c: Vec<f64> = p[n / 10..n - n / 10].to_vec();
        c.sort_by(|a, b| a.total_cmp(b));
        let f = c[(c.len() as f64 * 0.15) as usize] * 1.15;
        for v in &mut self.floor {
            if !self.floor_init || f < *v {
                *v = f;
            } else {
                *v = (*v * 10f64.powf(0.02 * dt)).min(f);
            }
        }
        self.floor_init = true;
        let bin_hz = self.fs / n as f64;
        let k_of =
            |hz: f64| ((hz / bin_hz).round() as i64 + n as i64 / 2).clamp(0, n as i64 - 1) as usize;
        let window = |off_hz: f64| -> (f64, f64, f64) {
            let (a, b) = (k_of(off_hz - 5e6), k_of(off_hz + 5e6));
            let pw: f64 = p[a..=b].iter().sum::<f64>();
            let fl = self.floor[a] * (b - a + 1) as f64;
            let mut num = 0.0;
            let mut den = 0.0;
            for k in a..=b {
                let x = (p[k] - self.floor[k]).max(0.0);
                num += x * (k as f64 - n as f64 / 2.0) * bin_hz;
                den += x;
            }
            (pw, fl, if den > 0.0 { num / den - off_hz } else { 0.0 })
        };
        for r in &mut self.rows {
            let off = (r.mhz - self.centre_mhz) * 1e6;
            r.inside = off.abs() <= self.fs / 2.0 - 5e6;
            if r.inside {
                let (pw, fl, _) = window(off);
                r.level_dbfs = db10(pw);
                r.over_db = r.level_dbfs - db10(fl);
                r.present = r.over_db >= 8.0;
                r.heard_s = Some(now_s);
            }
        }
        let tuned = fpv_nearest(&self.channels.list, self.centre_mhz, 4.0);
        let off = tuned
            .as_ref()
            .map_or(0.0, |c| (c.mhz - self.centre_mhz) * 1e6);
        let (pw, fl, o) = window(off);
        self.report.tuned = tuned.as_ref().map(|c| c.name.clone()).unwrap_or_default();
        self.report.tuned_mhz = tuned.map_or(self.centre_mhz, |c| c.mhz);
        self.report.level_dbfs = db10(pw);
        self.report.over_db = db10(pw) - db10(fl);
        self.report.floor_dbfs = db10(fl);
        self.report.offset_hz = o;
        self.report.table.clone_from(&self.rows);
        self.report.spectrum.clear();
        for i in 0..256 {
            let v = (p[4 * i] + p[4 * i + 1] + p[4 * i + 2] + p[4 * i + 3]) / 4.0;
            let mhz = ((4 * i) as f64 + 1.5 - n as f64 / 2.0) * bin_hz / 1e6;
            self.report.spectrum.push([mhz as f32, db10(v) as f32]);
        }
        self.hist_t += dt;
        if self.hist_t >= 1.0 {
            self.hist_t -= 1.0;
            if self.report.history.len() >= 120 {
                self.report.history.remove(0);
            }
            let x = self.report.history.len() as f32;
            self.report.history.push([x, self.report.level_dbfs as f32]);
        }
        self.acc.iter_mut().for_each(|v| *v = 0.0);
        self.n_acc = 0;
    }
}

impl Component for ChannelMeter {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "fpv.meter",
            vec![Port::iq("in")],
            vec![Port::typed::<MeterReport>("report").rate_from(RateFrom::Fixed(0.0))],
        )
        .with_settings(FpvChannelsSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.fs = inputs[0].rate;
        self.every = ((self.fs / 400.0).round() as u64).max(NFFT as u64);
        self.centre_mhz = inputs[0].centre_hz / 1e6;
        self.pos = 0;
        self.buf.clear();
        self.acc.iter_mut().for_each(|v| *v = 0.0);
        self.n_acc = 0;
        Ok(vec![Format::new(Kind::typed::<MeterReport>(), 0.0)])
    }
    fn process(&mut self, io: &mut Io) {
        for t in io.input(0).tags() {
            if let TagKind::Retuned { centre_hz, .. } = t.kind {
                self.centre_mhz = centre_hz / 1e6;
                self.acc.iter_mut().for_each(|v| *v = 0.0);
                self.n_acc = 0;
            }
        }
        // The first NFFT samples of every `every` go to a spectrum; the rest are skipped as a block.
        let x = io.iq(0);
        let mut i = 0;
        while i < x.len() {
            let slot = self.pos % self.every;
            let left = (x.len() - i) as u64;
            if slot >= NFFT as u64 {
                let skip = (self.every - slot).min(left);
                i += skip as usize;
                self.pos += skip;
                continue;
            }
            let take = (NFFT as u64 - slot).min(left) as usize;
            for &s in &x[i..i + take] {
                self.buf.push(s);
                if self.buf.len() == NFFT {
                    for k in 0..NFFT {
                        self.work[k] = self.buf[k] * self.win[k];
                    }
                    self.fft.process(&mut self.work);
                    for k in 0..NFFT {
                        self.acc[k] += self.work[k].norm_sqr() as f64;
                    }
                    self.n_acc += 1;
                    self.buf.clear();
                }
            }
            i += take;
            self.pos += take as u64;
        }
        if io.tick() {
            self.ticks += 1;
            let t = io.input_format(0).time_of(io.input(0).first());
            if self.ticks.is_multiple_of(10) && self.n_acc >= 8 {
                self.judge(0.1, t);
            }
            io.items_out::<MeterReport>(0).push(Timed {
                time: t,
                value: self.report.clone(),
            });
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        if self.channels.apply(id, v)? {
            self.rows = channel_rows(&self.channels.list);
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        self.channels.values()
    }
    fn commands(&self) -> &Commands {
        MeterCommand::commands()
    }
    fn invoke(&mut self, id: CommandId, _: &Values) -> Result<Option<Record>, CommandError> {
        match MeterCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            MeterCommand::Clear => self.clear(),
        }
        Ok(None)
    }
    /// The channel levels are a scan list, which the description of the stack keeps. The noise floor and the
    /// history are of the present frequency.
    fn forget(&mut self, _kept: &Kept) {
        self.floor_init = false;
        self.report.history.clear();
    }
}

/// Forgets the channel levels, the noise floor and the history.
pub const CLEAR: Command = Command::new("clear", "Clear the channel levels")
    .help("Forgets the channel levels, the noise floor and the level history.");

sdrkit_core::commands! {
    /// The commands of the channel meter.
    pub enum MeterCommand {
        /// Clear.
        Clear = CLEAR,
    }
}

/// The channel mixer: the channel nearest the radio's centre (within 4 MHz) to 0 Hz; the centre itself when none is near.
#[derive(Default)]
pub struct ChannelMix {
    nco: Option<Nco>,
    fs: f64,
    centre_hz: f64,
    channels: ChannelList,
}

impl ChannelMix {
    fn set_centre(&mut self, centre_hz: f64) {
        self.centre_hz = centre_hz;
        let off = if centre_hz > 0.0 {
            fpv_nearest(&self.channels.list, centre_hz / 1e6, 4.0)
                .map_or(0.0, |c| c.mhz * 1e6 - centre_hz)
        } else {
            0.0
        };
        // A channel at the centre needs no mixing.
        self.nco = (off != 0.0).then(|| Nco::new(self.fs, -off));
    }
}

impl Component for ChannelMix {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "fpv.chanmix",
            vec![Port::iq("in")],
            vec![Port::iq("out").same_format_as(0)],
        )
        .with_settings(FpvChannelsSetting::schema())
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        if self.channels.apply(id, v)? && self.fs > 0.0 {
            self.set_centre(self.centre_hz);
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        self.channels.values()
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.fs = inputs[0].rate;
        self.set_centre(inputs[0].centre_hz);
        Ok(vec![inputs[0].clone()])
    }
    fn process(&mut self, io: &mut Io) {
        for t in io.input(0).tags() {
            if let TagKind::Retuned { centre_hz, .. } = t.kind {
                self.set_centre(centre_hz);
            }
        }
        let x = io.iq(0);
        let out = io.iq_out(0);
        let s = out.len();
        out.extend_from_slice(x);
        if let Some(n) = &mut self.nco {
            n.mix_in_place(&mut out[s..]);
        }
    }
}

/// The sound subcarrier's report.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FpvSoundReport {
    /// Decoding.
    pub on: bool,
    /// Present.
    pub present: bool,
    /// The subcarrier in use, MHz.
    pub subcarrier_mhz: f64,
    /// Audio level, dBFS.
    pub level_dbfs: f64,
    /// Strength of 6.0 MHz, dB.
    pub s60_db: f64,
    /// Strength of 6.5 MHz, dB.
    pub s65_db: f64,
    /// Audio samples made.
    pub samples: u64,
}
impl Item for FpvSoundReport {
    const KIND: &'static str = "fpv.sound_report";
}

/// The sound subcarrier.
pub struct FpvSound {
    fs: f64,
    video_disc: sdrkit_algo_dsp::fm::Discriminator,
    /// Moves the band of the three subcarriers to 0 Hz.
    shift: Nco,
    /// Takes the band of the subcarriers down to `fs1`.
    band: Decimator<Complex32>,
    fs1: f64,
    mix: [Nco; 3],
    dec: Vec<Decimator<Complex32>>,
    p: [f64; 3],
    pn: usize,
    choice: SoundSubcarrier,
    on: bool,
    chosen: usize,
    present: bool,
    sound_disc: sdrkit_algo_dsp::fm::Discriminator,
    res: Option<Resampler<f32>>,
    fs2: f64,
    disc: Vec<f32>,
    a: Vec<Complex32>,
    c: Vec<Complex32>,
    b: [Vec<Complex32>; 3],
    m: Vec<f32>,
    o: Vec<f32>,
    rep: FpvSoundReport,
    lev: (f64, usize),
}

impl Default for FpvSound {
    fn default() -> Self {
        FpvSound {
            fs: 20e6,
            video_disc: crate::util::phase_steps(),
            shift: Nco::new(1.0, 0.0),
            band: Decimator::new(1.0, 1, 0.1, 50.0),
            fs1: 20e6,
            mix: [Nco::new(1.0, 0.0), Nco::new(1.0, 0.0), Nco::new(1.0, 0.0)],
            dec: Vec::new(),
            p: [0.0; 3],
            pn: 0,
            choice: SoundSubcarrier::Auto,
            on: true,
            chosen: 1,
            present: false,
            sound_disc: crate::util::phase_steps(),
            res: None,
            fs2: 250e3,
            disc: Vec::new(),
            a: Vec::new(),
            c: Vec::new(),
            b: [Vec::new(), Vec::new(), Vec::new()],
            m: Vec::new(),
            o: Vec::new(),
            rep: FpvSoundReport::default(),
            lev: (0.0, 0),
        }
    }
}

const SUBS: [f64; 3] = [6.0e6, 6.5e6, 7.1e6];

/// The centre of the band of [`SUBS`].
const BAND_HZ: f64 = 6.55e6;

/// The half width of that band: the outer subcarriers and the 100 kHz that their filters must keep free of aliases.
const BAND_HALF_HZ: f64 = 0.65e6;

/// The lowest rate of the band.
const BAND_RATE_HZ: f64 = 2.4e6;

impl Component for FpvSound {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "fpv.sound",
            vec![Port::iq("in")],
            vec![
                Port::audio("out").rate_from(RateFrom::Fixed(AUDIO_HZ)),
                Port::typed::<FpvSoundReport>("report").rate_from(RateFrom::Fixed(0.0)),
            ],
        )
        .with_settings(FpvSoundSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.fs = inputs[0].rate;
        let d = ((self.fs / 250e3).round() as usize).max(1);
        self.fs2 = self.fs / d as f64;
        // The band goes down by the largest factor of `d` that keeps its rate at BAND_RATE_HZ or more; each subcarrier
        // then by the rest.
        let m = (1..=d)
            .rev()
            .find(|&m| d.is_multiple_of(m) && self.fs / m as f64 >= BAND_RATE_HZ)
            .unwrap_or(1);
        self.fs1 = self.fs / m as f64;
        self.shift = Nco::new(self.fs, -BAND_HZ);
        self.band = if self.fs1 > 2.0 * BAND_HALF_HZ {
            Decimator::new(self.fs, m, BAND_HALF_HZ, 60.0)
        } else {
            Decimator::new(self.fs, 1, 0.1, 50.0)
        };
        self.mix = SUBS.map(|f| Nco::new(self.fs1, BAND_HZ - f));
        let alias_free = (self.fs2 - 100e3).max(80e3).min(self.fs2 - 70e3);
        self.dec = (0..3)
            .map(|_| {
                if self.fs > 2.0 * 7.2e6 {
                    Decimator::with_alias_free(self.fs1, d / m, 70e3, alias_free, 50.0)
                } else {
                    Decimator::new(1.0, 1, 0.1, 50.0)
                }
            })
            .collect();
        self.res = Resampler::new(self.fs2, AUDIO_HZ).ok();
        Ok(vec![
            Format::audio(AUDIO_HZ, 1),
            Format::new(Kind::typed::<FpvSoundReport>(), 0.0),
        ])
    }
    fn process(&mut self, io: &mut Io) {
        let x = io.iq(0);
        if !self.on || self.fs < 2.0 * 7.2e6 {
            if io.tick() {
                let t = io.input_format(0).time_of(io.input(0).first());
                io.items_out::<FpvSoundReport>(1).push(Timed {
                    time: t,
                    value: FpvSoundReport {
                        on: false,
                        ..self.rep
                    },
                });
            }
            return;
        }
        // The video discriminator at the full rate (radians a sample), as a real signal.
        self.disc.clear();
        self.video_disc.process(x, &mut self.disc);
        // The band of the subcarriers at 0 Hz and fs1, then each subcarrier at 0 Hz and fs2.
        self.a.clear();
        self.a.resize(self.disc.len(), Complex32::default());
        self.shift.mix_real(&self.disc, &mut self.a);
        self.c.clear();
        self.band.process(&self.a, &mut self.c);
        for k in 0..3 {
            self.a.clear();
            self.a.extend_from_slice(&self.c);
            self.mix[k].mix_in_place(&mut self.a);
            self.b[k].clear();
            self.dec[k].process(&self.a, &mut self.b[k]);
        }
        // Sample by sample, so the judgements fall on the same samples however the input is cut.
        self.m.clear();
        let k = self.fs2 / TAU / 50e3;
        for i in 0..self.b[0].len() {
            for c in 0..3 {
                self.p[c] += self.b[c][i].norm_sqr() as f64;
            }
            self.pn += 1;
            if self.pn as f64 >= 0.1 * self.fs2 {
                let r = self.p[2].max(1e-30);
                let s60 = db10(self.p[0] / (r * (6.0f64 / 7.1).powi(2)));
                let s65 = db10(self.p[1] / (r * (6.5f64 / 7.1).powi(2)));
                self.rep.s60_db = s60;
                self.rep.s65_db = s65;
                match self.choice {
                    SoundSubcarrier::Auto => {
                        self.chosen = if s65 >= s60 { 1 } else { 0 };
                        self.present = s60.max(s65) >= 6.0;
                    }
                    SoundSubcarrier::S60 => {
                        self.chosen = 0;
                        self.present = true;
                    }
                    SoundSubcarrier::S65 => {
                        self.chosen = 1;
                        self.present = true;
                    }
                }
                self.p = [0.0; 3];
                self.pn = 0;
            }
            // FM demodulation of the chosen subcarrier: 50 kHz full scale; silence while none is there.
            let s = self.b[self.chosen][i];
            let v = (self.sound_disc.process_sample(s) as f64 * k) as f32;
            self.m.push(if self.present { v } else { 0.0 });
        }
        self.o.clear();
        if let Some(r) = &mut self.res {
            r.process(&self.m, &mut self.o);
        }
        io.audio_out(0).samples.extend_from_slice(&self.o);
        for &v in &self.o {
            self.lev.0 += (v as f64).powi(2);
            self.lev.1 += 1;
            self.rep.samples += 1;
        }
        if self.lev.1 >= 4800 {
            self.rep.level_dbfs = db10(self.lev.0 / self.lev.1 as f64);
            self.lev = (0.0, 0);
        }
        if io.tick() {
            self.rep.on = true;
            self.rep.present = self.present;
            self.rep.subcarrier_mhz = SUBS[self.chosen] / 1e6;
            let t = io.input_format(0).time_of(io.input(0).first());
            io.items_out::<FpvSoundReport>(1).push(Timed {
                time: t,
                value: self.rep,
            });
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match FpvSoundSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            FpvSoundSetting::SoundSubcarrier => self.choice = SOUND_SUBCARRIER.read(id, v)?,
            FpvSoundSetting::Audio => self.on = AUDIO.read(id, v)?,
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([
            (
                FpvSoundSetting::SoundSubcarrier,
                Value::Choice(self.choice.index()),
            ),
            (FpvSoundSetting::Audio, Value::Bool(self.on)),
        ])
    }
}

sdrkit_core::settings! {
    /// The settings of the FPV sound.
    pub enum FpvSoundSetting {
        /// The subcarrier played.
        SoundSubcarrier = SOUND_SUBCARRIER,
        /// Sound on or off.
        Audio = AUDIO,
    }
}

/// The status snapshot of the `fpv` stack at each report. The results fields are written from it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FpvDetail {
    /// 0, 1 or 2.
    pub state: u8,
    /// The meter.
    pub meter: MeterReport,
    /// Sound.
    pub sound: FpvSoundReport,
    /// Lines (0 unknown).
    pub lines: u32,
    /// System name.
    pub system: String,
    /// Colour system.
    pub colour_system: Option<crate::systems::Colour>,
    /// Fields a second.
    pub field_hz: f64,
    /// Sync quality.
    pub sync_quality: f64,
    /// Video SNR.
    pub video_snr_db: f64,
    /// Pictures decoded.
    pub pictures: u64,
    /// Fields ok.
    pub fields_ok: u64,
    /// Fields damaged.
    pub fields_bad: u64,
    /// The polarity.
    pub polarity: FmPolarity,
    /// Names of the channels at the tuned frequency.
    pub names: String,
    /// Summary.
    pub summary: String,
}

sdrkit_core::choice_enum! {
    /// The FPV plans a scan visits.
    pub enum ScanPlan {
        /// The 5.8 GHz bands.
        G58 = ("5.8", "5.8 GHz"),
        /// The 1.2 GHz band.
        G12 = ("1.2", "1.2 GHz"),
    }
}

sdrkit_core::commands! {
    /// The commands of the FPV display.
    pub enum DisplayCommand {
        /// Tunes to a channel.
        ChannelTune = FPV_CHANNEL_TUNE.command,
        /// Scans a plan.
        Scan = FPV_SCAN.command,
    }
}

/// The FPV display: the results, and the requests to tune that its commands make.
pub struct FpvDisplay {
    published: Published,
    series: [Option<SeriesId>; 2],
    detail: FpvDetail,
    requests: Vec<HostRequest>,
    ticks: u64,
    meter: MeterReport,
    sync: SyncReport,
    pic: PictureReport,
    sound: FpvSoundReport,
    was: State,
    channels: ChannelList,
}

impl Default for FpvDisplay {
    fn default() -> Self {
        FpvDisplay {
            published: fpv_published(),
            series: [None; 2],
            detail: FpvDetail::default(),
            requests: Vec::new(),
            ticks: 0,
            meter: MeterReport::default(),
            sync: SyncReport::default(),
            pic: PictureReport::default(),
            sound: FpvSoundReport::default(),
            was: State::Searching,
            channels: ChannelList::default(),
        }
    }
}

impl FpvDisplay {
    /// Requests the host to tune to the channel whose `channels` row key is `row`.
    fn tune(&mut self, row: &str) -> Result<(), CommandError> {
        let Some(ch) = self.channels.list.iter().find(|c| c.name == row) else {
            return Err(CommandError::NoSuchRow {
                table: crate::results::table_id(&self.published, &FPV_CHANNELS),
                key: row.to_owned(),
            });
        };
        self.requests.push(HostRequest::Retune {
            tuned_hz: ch.mhz * 1e6,
            reason: format!("FPV channel {}", ch.name),
        });
        Ok(())
    }
}

impl Component for FpvDisplay {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "fpv.display",
            vec![
                Port::typed::<MeterReport>("meter"),
                Port::typed::<SyncReport>("sync"),
                Port::typed::<PictureReport>("picture"),
                Port::typed::<FpvSoundReport>("sound"),
            ],
            vec![
                Port::new("events", Kind::Messages).rate_from(RateFrom::Fixed(0.0)),
                Port::typed::<HostRequest>("requests").rate_from(RateFrom::Fixed(0.0)),
            ],
        )
        .with_settings(FpvChannelsSetting::schema())
    }
    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let requests = std::mem::take(&mut self.requests);
        let channels = std::mem::take(&mut self.channels);
        *self = FpvDisplay::default();
        self.requests = requests;
        self.channels = channels;
        self.series = ["spectrum", "history"].map(|k| series_id(&self.published, k));
        Ok(vec![
            Format::new(Kind::Messages, 0.0),
            Format::new(Kind::typed::<HostRequest>(), 0.0),
        ])
    }
    fn process(&mut self, io: &mut Io) {
        for r in self.requests.drain(..) {
            io.items_out::<HostRequest>(1).push(Timed {
                time: self.ticks as f64 * TICK_S,
                value: r,
            });
        }
        if let Some(m) = io.items::<MeterReport>(0).last() {
            self.meter.clone_from(&m.value);
        }
        if let Some(s) = last::<SyncReport>(io, 1) {
            self.sync = s;
        }
        if let Some(p) = io.items::<PictureReport>(2).last() {
            self.pic.clone_from(&p.value);
        }
        if let Some(s) = last::<FpvSoundReport>(io, 3) {
            self.sound = s;
        }
        if !io.tick() {
            return;
        }
        self.ticks += 1;
        if !self.ticks.is_multiple_of(10) {
            return;
        }
        let d = &mut self.detail;
        let vstate = self.sync.state;
        d.state = if vstate == 2 {
            2
        } else if vstate == 1 || self.meter.over_db >= 8.0 {
            1
        } else {
            0
        };
        d.meter.clone_from(&self.meter);
        d.sound = self.sound;
        d.lines = self.sync.lines;
        d.colour_system = self.pic.colour;
        let sys = match self.sync.lines {
            525 => crate::systems::System::get(crate::systems::SystemId::M),
            _ => crate::systems::System::get(crate::systems::SystemId::G),
        };
        d.system.clear();
        if let (Some(c), true) = (self.pic.colour, self.sync.lines > 0) {
            d.system = sys.name(c);
        }
        d.field_hz = if self.sync.lines > 0 {
            2.0 * self.sync.line_hz / f64::from(self.sync.lines)
        } else {
            0.0
        };
        d.sync_quality = self.sync.quality;
        let sigma = if self.sync.gain > 0.0 {
            self.sync.noise_var.sqrt() / self.sync.gain
        } else {
            1.0
        };
        d.video_snr_db = (20.0 * (1.0 / sigma.max(1e-9)).log10()).clamp(0.0, 60.0);
        d.pictures = self.pic.pictures;
        d.fields_ok = self.pic.fields_ok;
        d.fields_bad = self.pic.fields_bad;
        d.polarity = if vstate == 0 {
            FmPolarity::Trying
        } else {
            FmPolarity::TipLow
        };
        d.names = fpv_names_at(&self.channels.list, self.meter.tuned_mhz);
        let ch = if self.meter.tuned.is_empty() {
            String::new()
        } else {
            format!(" {} {:.0} MHz", self.meter.tuned, self.meter.tuned_mhz)
        };
        d.summary = match d.state {
            2 => format!(
                "FPV video{ch}: {}, video SNR {:.1} dB, {:.0} dB over noise",
                d.system, d.video_snr_db, self.meter.over_db
            ),
            1 => format!(
                "FPV video{ch}: transmitter found, {:.0} dB over noise, waiting for sync",
                self.meter.over_db
            ),
            _ => "FPV video: no transmitter on this channel".into(),
        };
        publish_fpv(&mut self.published, &self.detail);
        let [spectrum, history] = self.series;
        write_series(&mut self.published, spectrum, &self.meter.spectrum);
        write_series(&mut self.published, history, &self.meter.history);
        let d = &self.detail;
        let st = State::of(d.state);
        self.published.results_mut().set_status(&Status {
            lock_state: st.lock_state(),
            snr_db: Some(d.video_snr_db as f32),
            level_dbfs: Some(self.meter.level_dbfs as f32),
            offset_hz: Some(self.meter.offset_hz),
            summary: d.summary.clone(),
        });
        if st != self.was {
            io.messages_out(0).push(FPV_STATE.event(
                self.ticks as f64 * TICK_S,
                0.0,
                &FpvStateEvent {
                    state: st,
                    text: d.names.as_str(),
                    family: Family::Receiver,
                },
            ));
            self.was = st;
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        self.channels.apply(id, v)?;
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        self.channels.values()
    }
    fn results(&self) -> Option<&Results> {
        Some(self.published.results())
    }
    fn commands(&self) -> &Commands {
        DisplayCommand::commands()
    }
    fn invoke(&mut self, id: CommandId, args: &Values) -> Result<Option<Record>, CommandError> {
        match DisplayCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            DisplayCommand::ChannelTune => {
                let FpvChannelArgs { row } = FPV_CHANNEL_TUNE.args(args)?;
                self.tune(row)?;
            }
            DisplayCommand::Scan => {
                let FpvScanArgs { plan } = FPV_SCAN.args(args)?;
                let channels = fpv_scan_centres(&self.channels.list, plan == ScanPlan::G12)
                    .into_iter()
                    .map(|mhz| mhz * 1e6)
                    .collect();
                self.requests.push(HostRequest::Hop {
                    channels,
                    dwell_s: SCAN_DWELL_S,
                    stop_on_lock: false,
                });
            }
        }
        Ok(None)
    }
}

/// How long a scan stays on each centre, seconds.
pub const SCAN_DWELL_S: f64 = 0.7;

/// The lowest rate.
pub const MIN_RATE_HZ: f64 = 16e6;

/// Adds the `fpv.*` components to `r`.
pub fn register(r: &mut sdrkit_graph::Registry) {
    r.add("fpv.meter", || Box::new(ChannelMeter::default()));
    r.add("fpv.chanmix", || Box::new(ChannelMix::default()));
    r.add("fpv.sound", || Box::new(FpvSound::default()));
    r.add("fpv.display", || Box::new(FpvDisplay::default()));
}

/// The FPV graph for a system.
///
/// # Errors
///
/// [`GraphError`] if the graph does not build.
pub fn graph(system: FpvSystem, catalog: &Arc<Catalog>) -> Result<Graph, GraphError> {
    let mut sync = VideoSync::default();
    sync.fm = true;
    let mut pic = PictureDecoder::default();
    pic.fm = true;
    let mut sup = Supervisor::new(Modulation::FmVideo);
    match system {
        FpvSystem::Pal => sup.set_system(crate::settings::SystemChoice::G),
        FpvSystem::Ntsc => sup.set_system(crate::settings::SystemChoice::M),
        FpvSystem::Auto => {}
    }
    // The FM supervisor requires a monitor spectrum: a search component that only watches stands
    // in.
    let mut search = crate::atv::search::CarrierSearch::default();
    search.monitor_only = true;
    search.whole = true;
    let b = GraphBuilder::new()
        .add("meter", ChannelMeter::default())
        .add("chanmix", ChannelMix::default())
        .add("fmvideo", FmVideoDisc::default())
        .add("sync", sync)
        .add("colour", crate::atv::colour::ColourLock::default())
        .add("picture", pic)
        .add("supervisor", sup)
        .add("sound", FpvSound::default())
        .add("display", FpvDisplay::default())
        .add("teletext", crate::atv::teletext::TeletextDecoder::default())
        .add("snow", {
            let mut c = crate::atv::snow::Snow::default();
            c.fm = true;
            c
        })
        .add("monitor", search)
        .input("meter.in")
        .input("chanmix.in")
        .connect("chanmix.out", "fmvideo.in")
        .connect("chanmix.out", "sound.in")
        .connect("fmvideo.out", "sync.in")
        .input("supervisor.iq")
        .connect("sync.video", "teletext.video")
        .connect("sync.lines", "teletext.lines")
        .feedback("supervisor.tune", "teletext.control")
        .connect("chanmix.out", "snow.in")
        .connect("snow.out", "picture.snow")
        .feedback("supervisor.tune", "snow.control")
        .input("monitor.in")
        .input("monitor.raw")
        .connect("monitor.monitor", "supervisor.monitor")
        .connect("sync.report", "supervisor.sync")
        .connect("meter.report", "display.meter")
        .connect("sync.report", "display.sync")
        .connect("picture.report", "display.picture")
        .connect("sound.report", "display.sound")
        .feedback("supervisor.tune", "sync.control")
        .feedback("supervisor.tune", "monitor.control");
    // The display reads the counts of the picture decoder: it runs in the stage of the picture decoder.
    let b = crate::atv::graph::picture_chain(b);
    crate::transmit::branch(
        b,
        crate::transmit::FpvTransmit::default(),
        Some("sound.out"),
    )
    .output("display.events")
    .output("display.requests")
    .output("picture.out")
    .output("teletext.pages")
    .status_from("display")
    .clock(TICK_S)
    .reconfigure_on_retune()
    .costs(&sdrkit_core::CostTable::parse(crate::COSTS), "fpv", "-")
    .catalog(catalog)
    .build()
}

/// The FPV receiver: the graph of the system that `cfg` chooses, with its settings applied. The channel table is kept across
/// retunes. A change of system needs a new receiver (the setting says `restart`).
///
/// # Errors
///
/// [`GraphError`] if the graph does not build or `cfg` holds a value that a component refuses.
pub fn receiver(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    let mut b = StackGraph::of(
        crate::typed::shape("fpv"),
        graph(cfg.settings.get(&FPV_SYSTEM), &Catalog::of(cfg)?)?,
    )
    .min_rate(
        MIN_RATE_HZ,
        "FPV video needs a sample rate of at least 16 Msps",
    )
    .route(FPV_SYSTEM.key(), &[])?
    .route(
        CHANNELS.key(),
        &["meter.channels", "chanmix.channels", "display.channels"],
    )?
    .route(SOUND_SUBCARRIER.key(), &["sound.sound_subcarrier"])?
    .route(AUDIO.key(), &["sound.audio"])?
    .route(COLOUR_ON.key(), &["colour.colour_on"])?
    .route(SATURATION.key(), &["picture.saturation"])?
    .route(DEINTERLACE.key(), &["picture.deinterlace"])?
    .command("clear", sdrkit_graph::CommandTarget::of("meter", "clear"))
    .command(
        "channel.tune",
        sdrkit_graph::CommandTarget::of("display", "channel.tune"),
    )
    .command("scan", sdrkit_graph::CommandTarget::of("display", "scan"))
    .command(
        "teletext.select",
        sdrkit_graph::CommandTarget::of("teletext", "teletext.select"),
    );
    for s in crate::settings::FpvTxSetting::schema().settings() {
        let k = &s.field.key;
        b = b.route(k, &[&format!("transmit.{k}")])?;
    }
    for c in crate::transmit::COMMANDS {
        b = b.command(c, sdrkit_graph::CommandTarget::of("transmit", c));
    }
    b.with_settings(&cfg.settings).build()
}

/// The FPV generator's parameters.
#[derive(Clone, Debug)]
pub struct FpvGenParams {
    /// Output rate.
    pub rate_hz: f64,
    /// NTSC 525 (else PAL 625).
    pub ntsc: bool,
    /// Colour bars (else the test card).
    pub bars: bool,
    /// Sound subcarrier, Hz (0: none).
    pub sound_hz: f64,
    /// The subcarrier carries the host's audio ([`FpvGenerator::push_programme`]): full scale is the full deviation of
    /// 25 kHz. Without it, the 1 kHz test tone.
    pub host_sound: bool,
    /// No transmitter (silence: the framework's noise produces an empty channel).
    pub none: bool,
    /// The transmitter's place from the capture centre, Hz.
    pub centre_hz: f64,
    /// Peak deviation, Hz (sync tip at minus, white at plus).
    pub deviation_hz: f64,
    /// Sync tip at the highest frequency.
    pub sync_high: bool,
    /// AC coupling time constant, s (0 DC coupled).
    pub ac_s: f64,
    /// Amplitude.
    pub amplitude: f64,
}

impl Default for FpvGenParams {
    fn default() -> Self {
        FpvGenParams {
            rate_hz: 20e6,
            ntsc: false,
            bars: false,
            sound_hz: 6.5e6,
            host_sound: false,
            none: false,
            centre_hz: 0.0,
            deviation_hz: 8e6,
            sync_high: false,
            ac_s: 0.0,
            amplitude: 0.3,
        }
    }
}

/// The FPV modulator: the test signal of `fpv` (also the `atv` FM-video test signal) and the core of the `fpv` sender, which
/// gives it the pictures ([`FpvGenerator::show`]) and the sound ([`FpvGenerator::push_programme`]) of the host.
pub struct FpvGenerator {
    p: FpvGenParams,
    video: crate::gen_video::VideoGen,
    host_audio: crate::sound_gen::HostAudio,
    prog: f64,
    prog_next: f64,
    res: FractionalResampler<f32>,
    vbuf: Vec<f32>,
    pending: Vec<f32>,
    rd: usize,
    k: u64,
    phase: f64,
    sub_phase: f64,
    ac: f64,
    s: f64,
}

impl FpvGenerator {
    /// A generator.
    pub fn new(p: FpvGenParams) -> Self {
        let sys = if p.ntsc {
            crate::systems::SystemId::M
        } else {
            crate::systems::SystemId::G
        };
        let colour = if p.ntsc {
            crate::systems::Colour::Ntsc
        } else {
            crate::systems::Colour::Pal
        };
        let video = crate::gen_video::VideoGen::new(&crate::gen_video::VideoOptions {
            system: sys,
            colour,
            pattern: if p.bars {
                crate::card::Pattern::Bars
            } else {
                crate::card::Pattern::Card
            },
            group_delay: false,
            ..Default::default()
        });
        let fi = video.rate();
        let res = FractionalResampler::new(
            fi,
            p.rate_hz,
            5.0e6,
            (fi / 2.0).min(p.rate_hz - 5.0e6).max(5.5e6),
            50.0,
        )
        .unwrap_or_else(|_| FractionalResampler::with_rates(fi, p.rate_hz).expect("rates"));
        let s = video.raster().sync_level;
        FpvGenerator {
            video,
            host_audio: crate::sound_gen::HostAudio::default(),
            prog: 0.0,
            prog_next: 0.0,
            res,
            vbuf: vec![0.0; 4096],
            pending: Vec::new(),
            rd: 0,
            k: 0,
            phase: 0.0,
            sub_phase: 0.0,
            ac: 0.0,
            s,
            p,
        }
    }
}

impl FpvGenerator {
    /// The raster.
    pub fn raster(&self) -> &crate::systems::Raster {
        self.video.raster()
    }
    /// Shows a picture of the host from the next field on (`None`: the test picture again); see
    /// [`crate::gen_video::VideoGen::show`].
    pub fn show(&mut self, picture: Option<crate::card::Planes>) {
        self.video.show(picture);
    }
    /// The pictures of the host taken so far.
    pub fn pictures_taken(&self) -> u64 {
        self.video.pictures_taken()
    }
    /// The rate of the programme that [`FpvGenerator::push_programme`] takes: the output rate over
    /// [`crate::sound_gen::PROGRAMME_STEP`].
    pub fn programme_rate(&self) -> f64 {
        self.p.rate_hz / crate::sound_gen::PROGRAMME_STEP as f64
    }
    /// The programme samples that the next `n` output samples read.
    pub fn programme_due(&self, n: usize) -> usize {
        crate::sound_gen::programme_due(self.k, n)
    }
    /// Adds host audio at [`FpvGenerator::programme_rate`] for a generator made with `host_sound`.
    pub fn push_programme(&mut self, x: &[f32]) {
        self.host_audio.push(x);
    }
}

impl Generator for FpvGenerator {
    fn rate_hz(&self) -> Hz {
        self.p.rate_hz
    }
    /// The FM carrier has a constant envelope: its power is the stated power, in a bandwidth of 17 MHz.
    fn signal_info(&self) -> Option<sdrkit_core::SignalInfo> {
        Some(sdrkit_core::SignalInfo::new(17e6).with_power(self.p.amplitude * self.p.amplitude))
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        if self.p.none {
            out.fill(Complex32::default());
            return out.len();
        }
        let dt = 1.0 / self.p.rate_hz;
        for o in out.iter_mut() {
            while self.rd >= self.pending.len() {
                self.video.fill(&mut self.vbuf);
                if self.rd > 0 {
                    self.pending.drain(..self.rd);
                    self.rd = 0;
                }
                self.res.process(&self.vbuf, &mut self.pending);
            }
            let v = self.pending[self.rd] as f64;
            self.rd += 1;
            let t = self.k as f64 * dt;
            // The composite with the sound subcarrier (25 kHz deviation, 0.12 of the swing, a 1 kHz tone with a break every 2 s).
            let mut comp = (v - self.s) / (1.0 - self.s);
            if self.p.sound_hz > 0.0 {
                let step = crate::sound_gen::PROGRAMME_STEP;
                let tone = if self.p.host_sound {
                    if self.k.is_multiple_of(step) {
                        self.prog = self.prog_next;
                        self.prog_next = self.host_audio.take();
                    }
                    let frac = (self.k % step) as f64 / step as f64;
                    self.prog + (self.prog_next - self.prog) * frac
                } else if t % 2.0 < 1.9 {
                    (TAU * 1000.0 * t).sin()
                } else {
                    0.0
                };
                self.sub_phase = (self.sub_phase + (self.p.sound_hz + 25e3 * tone) * dt).fract();
                comp += 0.12 * (TAU * self.sub_phase).sin();
            }
            if self.p.ac_s > 0.0 {
                self.ac += (comp - self.ac) * dt / self.p.ac_s;
                comp = comp - self.ac + 0.5;
            }
            let mut f = -self.p.deviation_hz + 2.0 * self.p.deviation_hz * comp;
            if self.p.sync_high {
                f = -f;
            }
            f += self.p.centre_hz;
            self.phase = (self.phase + f * dt).rem_euclid(1.0);
            let (sn, cs) = sdrkit_algo_dsp::approx::sin_cos(TAU * self.phase);
            *o = Complex32::new(cs, sn) * self.p.amplitude as f32;
            self.k += 1;
        }
        out.len()
    }
}
