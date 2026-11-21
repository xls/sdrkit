//! The ACARS front end: the carrier finder (a spectrum every 8 ms on the 25 kHz grid or the channel list) and the pool of
//! channel decoders it starts, holds and stops.

use super::demod::ChannelDecoder;
use super::{ChannelEvent, ChannelLook, FrontState, RawBlock, StopReason, MIN_RATE_HZ};
use super::{CHANNELS, THRESHOLD_DB};
use crate::history::History;
use crate::report::freq_list;
use sdrkit_algo_dsp::fft::Fft;
use sdrkit_core::{Complex32, ConfigError, SettingError, SettingId, SettingSet, Value, Values};
use sdrkit_graph::{
    values_of, Applied, Component, ComponentInfo, Format, Io, Port, RateFrom, SlicePool,
};
use std::sync::Arc;

/// The VHF band ACARS channels lie in, Hz.
pub const BAND_HZ: (f64, f64) = (117.975e6, 137.0e6);

/// Reads the `channels` setting: frequencies in MHz inside [`BAND_HZ`].
///
/// # Errors
///
/// The byte position of the first word that is not such a frequency.
pub fn channel_list(t: &str) -> Result<Vec<f64>, usize> {
    freq_list(t, 1e6, BAND_HZ)
}

sdrkit_core::settings! {
    /// The settings of [`Front`].
    pub enum FrontSetting {
        /// The channel list.
        Channels = CHANNELS,
        /// The carrier threshold.
        ThresholdDb = THRESHOLD_DB,
    }
}

#[derive(Clone, Debug, Default)]
struct Cand {
    freq: f64,
    off: f64,
    centre_bin: i64,
    half: i64,
    p: f64,
    seeded: bool,
    ratio_db: f64,
    snr_db: f64,
    level_db: f64,
    cfo: f64,
    active: bool,
    heard: bool,
    last_above: f64,
    since: f64,
    banned_until: f64,
    dec: Option<usize>,
}

/// The carrier finder and the decoder pool.
pub struct Front {
    channels: String,
    threshold_db: f64,
    rate: f64,
    centre: f64,
    /// Where the session is tuned, Hz.
    station: f64,
    refused: String,
    hist: History,
    hop: u64,
    nfft: usize,
    fft: Option<Fft>,
    win: Vec<f32>,
    buf: Vec<Complex32>,
    pow: Vec<f64>,
    scratch: Vec<f64>,
    noise: Option<f64>,
    looks: u64,
    cands: Vec<Cand>,
    decoders: Vec<Option<ChannelDecoder>>,
    max_decoders: usize,
    idx: u64,
    next_look: u64,
    frames_retired: u64,
    blocks: Vec<RawBlock>,
    events: Vec<ChannelEvent>,
    list: Arc<[f64]>,
    looks_pool: SlicePool<ChannelLook>,
    tmp: Vec<Complex32>,
}

impl Default for Front {
    fn default() -> Self {
        Front {
            channels: String::new(),
            threshold_db: THRESHOLD_DB.default,
            rate: 0.0,
            centre: 0.0,
            station: 0.0,
            refused: String::new(),
            hist: History::new(1),
            hop: 1,
            nfft: 1024,
            fft: None,
            win: Vec::new(),
            buf: Vec::new(),
            pow: Vec::new(),
            scratch: Vec::new(),
            noise: None,
            looks: 0,
            cands: Vec::new(),
            decoders: Vec::new(),
            max_decoders: 8,
            idx: 0,
            next_look: 0,
            frames_retired: 0,
            blocks: Vec::new(),
            events: Vec::new(),
            list: Arc::from(Vec::new()),
            looks_pool: SlicePool::default(),
            tmp: Vec::new(),
        }
    }
}

impl Front {
    fn build(&mut self) {
        self.cands.clear();
        self.decoders.clear();
        self.noise = None;
        self.looks = 0;
        self.refused.clear();
        if !(self.rate.is_finite() && self.rate >= MIN_RATE_HZ) {
            self.refused = format!(
                "sample rate {:.0} kHz is too low, use at least {:.0} kHz",
                if self.rate.is_finite() {
                    self.rate / 1e3
                } else {
                    0.0
                },
                MIN_RATE_HZ / 1e3
            );
            self.fft = None;
            return;
        }
        let rate = self.rate;
        self.hop = (0.008 * rate).round().max(1.0) as u64;
        let mut n = 1024usize;
        while rate / n as f64 > 1500.0 && n < 16384 {
            n *= 2;
        }
        self.nfft = n;
        self.fft = Some(Fft::forward(n));
        self.win = (0..n)
            .map(|i| (0.5 - 0.5 * (std::f64::consts::TAU * i as f64 / n as f64).cos()) as f32)
            .collect();
        self.buf = vec![Complex32::default(); n];
        self.pow = vec![0.0; n];
        self.hist = History::new((0.25 * rate) as usize + n);
        self.next_look = self.idx + self.hop;
        self.max_decoders = if rate > 4e6 { 16 } else { 8 };
        let binw = rate / n as f64;
        let half = ((3400.0 / binw).ceil() as i64).max(2);
        self.list = Arc::from(channel_list(&self.channels).unwrap_or_default());
        let mut freqs: Vec<(f64, bool)> = Vec::new();
        if self.list.is_empty() {
            // The grid inside the VHF band; a session tuned outside the band watches the grid where it is tuned.
            let (band_lo, band_hi) = if (BAND_HZ.0..=BAND_HZ.1).contains(&self.station) {
                (118e6, 137e6)
            } else {
                (f64::MIN, f64::MAX)
            };
            let lo = ((self.centre - 0.45 * rate).max(band_lo) / 25e3).ceil() as i64;
            let hi = ((self.centre + 0.45 * rate).min(band_hi) / 25e3).floor() as i64;
            for k in lo..=hi {
                let f = k as f64 * 25e3;
                if (f - self.centre).abs() >= 2.5e3 {
                    freqs.push((f, false));
                }
            }
        } else {
            for &f in self.list.iter() {
                if (f - self.centre).abs() <= 0.45 * rate {
                    freqs.push((f, true));
                }
            }
        }
        for (f, _) in freqs {
            let off = f - self.centre;
            self.cands.push(Cand {
                freq: f,
                off,
                centre_bin: (off / binw).round() as i64,
                half,
                level_db: -120.0,
                snr_db: -30.0,
                ..Default::default()
            });
        }
    }

    fn time(&self) -> f64 {
        self.idx as f64 / self.rate
    }

    fn stop(&mut self, ci: usize, reason: StopReason) {
        let c = &mut self.cands[ci];
        c.active = false;
        if let Some(d) = c.dec.take() {
            if let Some(mut dec) = self.decoders[d].take() {
                dec.flush();
                self.frames_retired += dec.frames_started();
                self.blocks.append(&mut dec.out);
            }
        }
        let c = &self.cands[ci];
        self.events.push(ChannelEvent {
            freq_hz: c.freq,
            offset_hz: c.off,
            start: false,
            cn_db: c.ratio_db as f32,
            cfo_hz: c.cfo as f32,
            reason: Some(reason),
        });
    }

    fn start(&mut self, ci: usize) {
        let slot = match self.decoders.iter().position(Option::is_none) {
            Some(s) => s,
            None => {
                self.decoders.push(None);
                self.decoders.len() - 1
            }
        };
        let now = self.time();
        let from = self
            .idx
            .saturating_sub((0.060 * self.rate) as u64)
            .max(self.hist.start());
        let c = &mut self.cands[ci];
        let Ok(mut d) = ChannelDecoder::new(self.rate, c.freq, c.off + c.cfo, from) else {
            return;
        };
        c.active = true;
        c.heard = true;
        c.since = now;
        c.last_above = now;
        c.dec = Some(slot);
        // Catch up from the history.
        let mut i = from;
        while i < self.idx {
            let e = (i + 65_536).min(self.idx);
            self.hist.copy(i, e, &mut self.tmp);
            d.feed(&self.tmp);
            i = e;
        }
        self.decoders[slot] = Some(d);
        let c = &self.cands[ci];
        self.events.push(ChannelEvent {
            freq_hz: c.freq,
            offset_hz: c.off,
            start: true,
            cn_db: c.ratio_db as f32,
            cfo_hz: c.cfo as f32,
            reason: None,
        });
    }

    fn look(&mut self) {
        let n = self.nfft;
        if self.hist.end() < n as u64 {
            return;
        }
        let e = self.hist.end();
        for (k, v) in self.buf.iter_mut().enumerate() {
            *v = self.hist.get(e - n as u64 + k as u64) * self.win[k];
        }
        if let Some(f) = self.fft.as_mut() {
            f.process(&mut self.buf);
        }
        for (p, v) in self.pow.iter_mut().zip(&self.buf) {
            *p = v.norm_sqr() as f64;
        }
        self.scratch.clear();
        self.scratch.extend_from_slice(&self.pow);
        let mid = n / 2;
        self.scratch
            .select_nth_unstable_by(mid, |a, b| a.total_cmp(b));
        let nb_now = self.scratch[mid] / std::f64::consts::LN_2;
        let noise = match self.noise {
            None => nb_now,
            Some(x) => 0.9 * x + 0.1 * nb_now,
        };
        self.noise = Some(noise);
        self.looks += 1;
        if self.looks <= 2 {
            return;
        }
        let now = self.time();
        let binw = self.rate / n as f64;
        let sumw = n as f64 / 2.0;
        let thr = self.threshold_db;
        let mut want: Vec<(f64, usize)> = Vec::new();
        for ci in 0..self.cands.len() {
            let c = &mut self.cands[ci];
            let (mut raw, mut wsum, mut fsum) = (0.0, 0.0, 0.0);
            for k in -c.half..=c.half {
                let b = (c.centre_bin + k).rem_euclid(n as i64) as usize;
                let p = self.pow[b];
                raw += p;
                let x = (p - noise).max(0.0);
                wsum += x;
                fsum += x * (k as f64 * binw + (c.centre_bin as f64 * binw - c.off));
            }
            let nb = (2 * c.half + 1) as f64;
            let noise_n = noise * nb;
            c.p = if c.seeded { 0.5 * c.p + 0.5 * raw } else { raw };
            c.seeded = true;
            let ratio = c.p / noise_n.max(1e-30);
            c.ratio_db = 10.0 * ratio.max(1e-12).log10();
            c.snr_db = (10.0 * ((c.p - noise_n) / noise_n.max(1e-30)).max(1e-3).log10()).max(-30.0);
            c.level_db = if c.ratio_db < thr - 3.0 {
                -120.0
            } else {
                10.0 * (c.p / (sumw * sumw)).max(1e-12).log10()
            };
            if raw >= 10.0 * noise_n && wsum > 0.0 {
                let cfo = fsum / wsum;
                c.cfo = if c.active {
                    0.7 * c.cfo + 0.3 * cfo
                } else {
                    cfo
                };
            }
            if c.active {
                if c.ratio_db > thr - 3.0 {
                    c.last_above = now;
                }
            } else if c.ratio_db >= thr && now >= c.banned_until {
                want.push((c.ratio_db, ci));
            }
        }
        // Stops.
        for ci in 0..self.cands.len() {
            let c = &self.cands[ci];
            if !c.active {
                continue;
            }
            let frames = c
                .dec
                .and_then(|d| self.decoders[d].as_ref())
                .map_or(0, |d| d.frames_started());
            if now - c.last_above > 0.4 {
                self.stop(ci, StopReason::Gone);
            } else if now - c.since > 8.0 && frames == 0 {
                self.stop(ci, StopReason::NotAcars);
                self.cands[ci].banned_until = now + 30.0;
            }
        }
        // Starts, strongest first.
        want.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
        for (r, ci) in want {
            let active = self.cands.iter().filter(|c| c.active).count();
            if active < self.max_decoders {
                self.start(ci);
                continue;
            }
            let weakest = self
                .cands
                .iter()
                .enumerate()
                .filter(|(_, c)| c.active)
                .filter(|(_, c)| {
                    c.dec
                        .and_then(|d| self.decoders[d].as_ref())
                        .is_some_and(|d| d.frames_started() == 0)
                })
                .min_by(|a, b| a.1.ratio_db.total_cmp(&b.1.ratio_db))
                .map(|(i, c)| (i, c.ratio_db));
            if let Some((wi, wr)) = weakest {
                if r >= wr + 3.0 {
                    self.stop(wi, StopReason::Preempted);
                    self.start(ci);
                }
            }
        }
        // Blocks finished by the decoders, in time order.
        let mut got: Vec<RawBlock> = Vec::new();
        for d in self.decoders.iter_mut().flatten() {
            got.append(&mut d.out);
        }
        got.sort_by(|a, b| {
            a.end_s
                .total_cmp(&b.end_s)
                .then(a.freq_hz.total_cmp(&b.freq_hz))
        });
        self.blocks.append(&mut got);
    }

    fn state(&mut self) -> FrontState {
        let n = self.cands.iter().filter(|c| c.heard || c.active).count();
        let cands = &self.cands;
        let channels = self.looks_pool.exact(n, |out| {
            let looks = cands.iter().filter(|c| c.heard || c.active);
            for (o, c) in out.iter_mut().zip(looks) {
                *o = ChannelLook {
                    freq_hz: c.freq,
                    level_db: c.level_db as f32,
                    snr_db: c.snr_db as f32,
                    cfo_hz: c.cfo as f32,
                    active: c.active,
                    heard: c.heard,
                };
            }
        });
        FrontState {
            time_s: if self.rate > 0.0 { self.time() } else { 0.0 },
            centre_hz: self.centre,
            rate_hz: self.rate,
            frames_started: self.frames_retired
                + self
                    .decoders
                    .iter()
                    .flatten()
                    .map(|d| d.frames_started())
                    .sum::<u64>(),
            channels,
            list: self.list.clone(),
            refused: self.refused.clone(),
        }
    }
}

impl Component for Front {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "avdata.acars_front",
            vec![Port::iq("in")],
            vec![
                Port::typed::<RawBlock>("blocks").rate_from(RateFrom::Fixed(0.0)),
                Port::typed::<ChannelEvent>("events").rate_from(RateFrom::Fixed(0.0)),
                Port::typed::<FrontState>("state").rate_from(RateFrom::Fixed(0.0)),
            ],
        )
        .with_settings(FrontSetting::schema())
    }

    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.rate = inputs[0].rate;
        self.centre = inputs[0].centre_hz;
        self.station = match inputs[0].tuned_hz {
            t if t.is_finite() && t > 0.0 => t,
            _ => self.centre,
        };
        self.idx = 0;
        self.frames_retired = 0;
        self.build();
        Ok(vec![
            Format::new(sdrkit_graph::Kind::typed::<RawBlock>(), 0.0),
            Format::new(sdrkit_graph::Kind::typed::<ChannelEvent>(), 0.0),
            Format::new(sdrkit_graph::Kind::typed::<FrontState>(), 0.0),
        ])
    }

    fn process(&mut self, io: &mut Io) {
        let x = io.iq(0);
        let tick = io.tick();
        if self.fft.is_some() {
            let mut i = 0;
            while i < x.len() {
                let n = ((self.next_look - self.idx) as usize).min(x.len() - i);
                let piece = &x[i..i + n];
                for &v in piece {
                    self.hist.push(v);
                }
                for d in self.decoders.iter_mut().flatten() {
                    d.feed(piece);
                }
                self.idx += n as u64;
                i += n;
                if self.idx == self.next_look {
                    self.look();
                    self.next_look += self.hop;
                }
            }
        }
        let t = if self.rate > 0.0 { self.time() } else { 0.0 };
        if !self.blocks.is_empty() {
            let o = io.items_out::<RawBlock>(0);
            for b in self.blocks.drain(..) {
                o.push(sdrkit_graph::Timed {
                    time: b.end_s,
                    value: b,
                });
            }
        }
        if !self.events.is_empty() {
            let o = io.items_out::<ChannelEvent>(1);
            for e in self.events.drain(..) {
                o.push(sdrkit_graph::Timed { time: t, value: e });
            }
        }
        if tick {
            let s = self.state();
            io.items_out::<FrontState>(2)
                .push(sdrkit_graph::Timed { time: t, value: s });
        }
    }

    fn apply(&mut self, id: SettingId, value: &Value) -> Result<Applied, SettingError> {
        match FrontSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            FrontSetting::Channels => {
                let t = CHANNELS.read(id, value)?;
                channel_list(t).map_err(|at| SettingError::Malformed { id, at: Some(at) })?;
                t.clone_into(&mut self.channels);
                if self.rate > 0.0 {
                    self.build();
                }
            }
            FrontSetting::ThresholdDb => self.threshold_db = THRESHOLD_DB.read(id, value)?,
        }
        Ok(Applied::Live)
    }

    fn values(&self) -> Values {
        values_of([
            (FrontSetting::Channels, Value::Text(self.channels.clone())),
            (FrontSetting::ThresholdDb, Value::F64(self.threshold_db)),
        ])
    }
}
