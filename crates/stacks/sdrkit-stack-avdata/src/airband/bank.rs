//! The channel bank: one receiver for each listed channel inside the band, and the estimate of the radio's tuning error from
//! their hold spectra.

use super::channel::{ChannelRx, ChannelStatus, HOLD_N};
use super::names::{list_check, list_from_text, Channel};
use super::{Activity, BankState, ChannelAudio, Gate, CHANNELS, HANG_S, MIN_RATE_HZ, SQUELCH_DB};
use sdrkit_core::publish::field;
use sdrkit_core::{ConfigError, Setting, SettingError, SettingId, SettingSet, Value, Values};
use sdrkit_graph::{
    values_of, Applied, Component, ComponentInfo, Format, Io, Kind, Port, RateFrom, SlicePool,
    Timed,
};

/// The radio's tuning error to start from, until the bank measures it.
pub const TUNE_ERROR_HZ: Setting<field::Hz> =
    Setting::new("tune_error_hz", "Tuning error guess", "Hz", 0.0)
        .range(-10_000.0, 10_000.0, 1.0)
        .help("The radio's tuning error to start from. Until it is set, the bank measures it.");

sdrkit_core::settings! {
    /// The settings of [`Bank`].
    pub enum BankSetting {
        /// The channel list.
        Channels = CHANNELS,
        /// The squelch.
        SquelchDb = SQUELCH_DB,
        /// The hang time.
        HangS = HANG_S,
        /// The tuning error to start from.
        TuneErrorHz = TUNE_ERROR_HZ,
    }
}

/// The test layout around a dial (`modeOpt` 0): Tower, Approach, Ground, ATIS, Tone.
pub fn test_layout(dial: f64) -> Vec<Channel> {
    let step = super::names::STEP_833;
    vec![
        Channel {
            freq_hz: dial,
            label: "Tower".into(),
            ..Default::default()
        },
        Channel {
            freq_hz: dial + step,
            is833: true,
            label: "Approach".into(),
            ..Default::default()
        },
        Channel {
            freq_hz: dial + 250e3,
            label: "Ground".into(),
            ..Default::default()
        },
        Channel {
            freq_hz: dial - 300e3,
            label: "ATIS".into(),
            ..Default::default()
        },
        Channel {
            freq_hz: dial + 191_666.67,
            is833: true,
            label: "Tone".into(),
            ..Default::default()
        },
    ]
}

/// The bank.
pub struct Bank {
    list_text: String,
    audio_pool: SlicePool<f32>,
    gate_pool: SlicePool<Gate>,
    squelch: f64,
    hang: f64,
    guess: Option<f64>,
    rate: f64,
    centre: f64,
    dial: f64,
    chans: Vec<ChannelRx>,
    list: Vec<Channel>,
    in_band: Vec<bool>,
    tune_err: f64,
    known: bool,
    next_est: f64,
    t: f64,
    log: Vec<String>,
    finished: Vec<Activity>,
    level: f64,
    level_n: u64,
}

impl Default for Bank {
    fn default() -> Self {
        Bank {
            list_text: String::new(),
            audio_pool: SlicePool::default(),
            gate_pool: SlicePool::default(),
            squelch: SQUELCH_DB.default,
            hang: HANG_S.default,
            guess: None,
            rate: 0.0,
            centre: 0.0,
            dial: 0.0,
            chans: Vec::new(),
            list: Vec::new(),
            in_band: Vec::new(),
            tune_err: 0.0,
            known: false,
            next_est: 0.5,
            t: 0.0,
            log: Vec::new(),
            finished: Vec::new(),
            level: 0.0,
            level_n: 0,
        }
    }
}

impl Bank {
    fn build(&mut self) {
        self.chans.clear();
        self.list = list_from_text(&self.list_text);
        if self.list.is_empty() && self.list_text.trim().is_empty() {
            self.list = test_layout(self.dial);
        }
        self.in_band = vec![false; self.list.len()];
        if !(self.rate.is_finite() && self.rate >= MIN_RATE_HZ) {
            return;
        }
        let half = 0.45 * self.rate;
        for (i, c) in self.list.iter().enumerate() {
            let off = c.freq_hz - self.centre;
            // Inside the band: the channel's edge inside the Nyquist band (the margin for the tuning error is taken from what is
            // left, so a channel near the edge of a narrow capture is still received).
            let inb = off.abs() + 12.5e3 + 12e3 < half || off.abs() + 12.5e3 < 0.5 * self.rate;
            self.in_band[i] = inb;
            if !inb {
                continue;
            }
            let st = ChannelStatus {
                freq_hz: c.freq_hz,
                name: c.name(),
                label: c.label.clone(),
                is833: c.is833,
                in_band: true,
                mute: c.mute,
                solo: c.solo,
                priority: c.priority,
                squelch_db: c.squelch_db,
                ..Default::default()
            };
            match ChannelRx::new(i, st, self.rate, off, self.tune_err) {
                Ok(c) => self.chans.push(c),
                Err(e) => self.log.push(format!("{}: {e}", c.name())),
            }
        }
        let n = self.chans.len();
        self.log.push(format!(
            "{n} of {} channels inside the band around {:.4} MHz, demodulated at {:.0} Hz",
            self.list.len(),
            self.centre / 1e6,
            self.chans
                .first()
                .map_or(0.0, |c| c.fi() / (c.fi() / 16e3).floor().max(1.0))
        ));
    }

    /// The lines of a channel's hold spectrum within +-12 kHz, strongest first: (Hz from the channel's nominal frequency, power).
    fn lines(&self, c: &ChannelRx) -> Vec<(f64, f64)> {
        let fi = c.fi();
        let n = HOLD_N as i64;
        let binw = fi / HOLD_N as f64;
        let h = &c.hold;
        let p = |k: i64| h[k.rem_euclid(n) as usize] as f64;
        let span = (12e3 / binw) as i64;
        let mut v: Vec<f64> = (-span..=span).map(p).collect();
        let mid = v.len() / 2;
        v.select_nth_unstable_by(mid, |a, b| a.total_cmp(b));
        let median = v[mid].max(1e-30);
        let dc = -(c.st.freq_hz - self.centre) - 0.0;
        let mut out: Vec<(f64, f64)> = Vec::new();
        let (s6, s14) = (6i64, 14i64);
        for k in -span..=span {
            let x = p(k);
            if x < 16.0 * median {
                continue;
            }
            if (-2..=2).any(|d| d != 0 && p(k + d) > x) {
                continue;
            }
            let side =
                |sgn: i64| (s6..=s14).map(|d| p(k + sgn * d)).sum::<f64>() / (s14 - s6 + 1) as f64;
            if x < 10.0 * side(1) || x < 10.0 * side(-1) {
                continue;
            }
            let f = k as f64 * binw;
            if (f - dc).abs() < 300.0 {
                continue;
            }
            out.push((f, x));
        }
        // The strongest few (the carrier, a strong neighbour, a tone's sidebands).
        out.sort_by(|a, b| b.1.total_cmp(&a.1));
        out.truncate(4);
        out
    }

    fn estimate(&mut self) {
        /// A channel's index, its lines (Hz, power) and its tuning tolerance.
        type ChannelLines = (usize, Vec<(f64, f64)>, f64);
        let lines: Vec<ChannelLines> = self
            .chans
            .iter()
            .enumerate()
            .filter(|(_, c)| c.hold_ready())
            .map(|(i, c)| (i, self.lines(c), c.tolerance()))
            .collect();
        let matched = |e: f64| -> Vec<f64> {
            let mut m = Vec::new();
            for (_, ls, tol) in &lines {
                if let Some(best) = ls
                    .iter()
                    .map(|l| l.0)
                    .filter(|f| (*f - e).abs() <= *tol)
                    .min_by(|a, b| (*a - e).abs().total_cmp(&(*b - e).abs()))
                {
                    m.push(best);
                }
            }
            m
        };
        let med = |mut v: Vec<f64>| {
            v.sort_by(|a, b| a.total_cmp(b));
            v[v.len() / 2]
        };
        let at_cur = matched(self.tune_err);
        // Every line is a candidate error: (channels agreeing, median of their lines, strength of the line).
        let mut cands: Vec<(usize, f64, f64)> = Vec::new();
        for (_, ls, _) in &lines {
            for &(f, pw) in ls {
                let m = matched(f);
                cands.push((m.len(), med(m), pw));
            }
        }
        let pick = |ok: &dyn Fn(&(usize, f64, f64)) -> bool| {
            cands
                .iter()
                .filter(|c| ok(c))
                .max_by(|a, b| a.0.cmp(&b.0).then(a.2.total_cmp(&b.2)))
                .copied()
        };
        let best = pick(&|c| c.0 >= 2 && c.0 > at_cur.len());
        let old = self.tune_err;
        let mut new = best.map(|b| b.1);
        // Before any measure, one channel alone will do when the error is under half an 8.33 kHz step (so a strong neighbour 8.33 kHz
        // away is never taken for the channel).
        if new.is_none() && !self.known {
            new = pick(&|c| c.0 >= 1 && c.1.abs() <= 4.1e3).map(|b| b.1);
        }
        if let Some(e) = new {
            self.tune_err = e.clamp(-12e3, 12e3);
            self.known = true;
        } else if !at_cur.is_empty() {
            let m = med(at_cur);
            self.tune_err += 0.3 * (m - self.tune_err);
            self.known = true;
        }
        if (self.tune_err - old).abs() > 1e3 {
            self.log.push(format!(
                "the radio is {:+.1} kHz off, measured from {} channels",
                self.tune_err / 1e3,
                matched(self.tune_err).len()
            ));
        }
        if self.tune_err != old {
            let e = self.tune_err;
            for c in self.chans.iter_mut() {
                c.set_tune_err(e);
            }
        }
    }
}

impl Component for Bank {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "avdata.airband_bank",
            vec![Port::iq("in")],
            vec![
                Port::typed::<ChannelAudio>("audio").rate_from(RateFrom::Fixed(0.0)),
                Port::typed::<BankState>("state").rate_from(RateFrom::Fixed(0.0)),
                Port::typed::<Activity>("activity").rate_from(RateFrom::Fixed(0.0)),
            ],
        )
        .with_settings(BankSetting::schema())
    }

    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let f = &inputs[0];
        self.rate = f.rate;
        self.centre = f.centre_hz;
        self.dial = f.tuned_hz;
        self.t = 0.0;
        self.next_est = 0.5;
        self.known = self.guess.is_some();
        self.tune_err = self.guess.unwrap_or(0.0);
        self.build();
        Ok(vec![
            Format::new(Kind::typed::<ChannelAudio>(), 0.0),
            Format::new(Kind::typed::<BankState>(), 0.0),
            Format::new(Kind::typed::<Activity>(), 0.0),
        ])
    }

    fn process(&mut self, io: &mut Io) {
        let x = io.iq(0);
        if self.rate.is_finite() && self.rate >= MIN_RATE_HZ {
            // Estimate the tuning error every 0.5 s of signal, at exact sample positions.
            let mut i = 0;
            while i < x.len() {
                let to_est = ((self.next_est - self.t) * self.rate).ceil().max(1.0) as usize;
                let n = to_est.min(x.len() - i);
                let piece = &x[i..i + n];
                for c in piece {
                    self.level += c.norm_sqr() as f64;
                }
                self.level_n += n as u64;
                let (sq, hang) = (self.squelch, self.hang);
                for c in self.chans.iter_mut() {
                    c.feed(piece, sq, hang);
                }
                // The audio of this piece, every channel the same length.
                let len = self.chans.iter().map(|c| c.audio.len()).min().unwrap_or(0);
                if len > 0 {
                    let chans = &self.chans;
                    let a = ChannelAudio {
                        len,
                        audio: self.audio_pool.exact(len * chans.len(), |o| {
                            for (dst, c) in o.chunks_exact_mut(len).zip(chans) {
                                dst.copy_from_slice(&c.audio[..len]);
                            }
                        }),
                        gates: self.gate_pool.exact(chans.len(), |o| {
                            for (g, c) in o.iter_mut().zip(chans) {
                                *g = Gate {
                                    freq_hz: c.st.freq_hz,
                                    open: c.st.open,
                                    mute: c.st.mute,
                                    solo: c.st.solo,
                                    priority: c.st.priority,
                                };
                            }
                        }),
                    };
                    io.items_out::<ChannelAudio>(0).push(Timed {
                        time: self.t,
                        value: a,
                    });
                }
                for c in self.chans.iter_mut() {
                    self.finished.append(&mut c.finished);
                }
                self.t += n as f64 / self.rate;
                i += n;
                if self.t + 1e-12 >= self.next_est {
                    self.next_est += 0.5;
                    self.estimate();
                }
            }
        }
        if !self.finished.is_empty() {
            let o = io.items_out::<Activity>(2);
            for a in self.finished.drain(..) {
                o.push(Timed {
                    time: a.start_s,
                    value: a,
                });
            }
        }
        if io.tick() {
            let mut status: Vec<ChannelStatus> = Vec::new();
            let mut ci = 0;
            for (i, c) in self.list.iter().enumerate() {
                if self.in_band.get(i).copied().unwrap_or(false)
                    && ci < self.chans.len()
                    && self.chans[ci].index == i
                {
                    let mut s = self.chans[ci].st.clone();
                    if s.open {
                        s.last_s = self.t;
                    }
                    status.push(s);
                    ci += 1;
                } else {
                    status.push(ChannelStatus {
                        freq_hz: c.freq_hz,
                        name: c.name(),
                        label: c.label.clone(),
                        is833: c.is833,
                        mute: c.mute,
                        solo: c.solo,
                        priority: c.priority,
                        squelch_db: c.squelch_db,
                        cn_db: -99.0,
                        level_db: -200.0,
                        ..Default::default()
                    });
                }
            }
            let level = if self.level_n > 0 {
                10.0 * (self.level / self.level_n as f64).max(1e-20).log10()
            } else {
                -200.0
            };
            self.level = 0.0;
            self.level_n = 0;
            let st = BankState {
                time_s: self.t,
                rate_hz: self.rate,
                centre_hz: self.centre,
                channel_rate_hz: self
                    .chans
                    .first()
                    .map_or(0.0, |c| c.fi() / (c.fi() / 16e3).floor().max(1.0)),
                tune_error_hz: self.tune_err,
                known: self.known,
                squelch_db: self.squelch,
                hang_s: self.hang,
                channels: status,
                level_db: level as f32,
                log: std::mem::take(&mut self.log),
                open_since: self
                    .chans
                    .iter()
                    .filter_map(|c| c.open_since().map(|t| (c.index, t, c.best_cn())))
                    .collect(),
            };
            let t = self.t;
            io.items_out::<BankState>(1)
                .push(Timed { time: t, value: st });
        }
    }

    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match BankSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            BankSetting::Channels => {
                let new = CHANNELS.read(id, v)?;
                list_check(new).map_err(|at| SettingError::Malformed { id, at: Some(at) })?;
                let old = list_from_text(&self.list_text);
                let nl = list_from_text(new);
                let same_freqs = old.len() == nl.len()
                    && old
                        .iter()
                        .zip(&nl)
                        .all(|(a, b)| a.freq_hz == b.freq_hz && a.is833 == b.is833);
                new.clone_into(&mut self.list_text);
                if same_freqs && !self.chans.is_empty() && !nl.is_empty() {
                    // Mute, solo, priority, label or squelch: no restart.
                    self.list = nl;
                    for c in self.chans.iter_mut() {
                        let l = &self.list[c.index];
                        c.st.mute = l.mute;
                        c.st.solo = l.solo;
                        c.st.priority = l.priority;
                        c.st.squelch_db = l.squelch_db;
                        c.st.label.clone_from(&l.label);
                    }
                } else if self.rate > 0.0 {
                    self.build();
                }
            }
            BankSetting::SquelchDb => self.squelch = SQUELCH_DB.read(id, v)?,
            BankSetting::HangS => self.hang = HANG_S.read(id, v)?,
            BankSetting::TuneErrorHz => {
                let g = TUNE_ERROR_HZ.read(id, v)?;
                self.guess = Some(g);
                self.tune_err = g;
                self.known = true;
                for c in self.chans.iter_mut() {
                    c.set_tune_err(g);
                }
            }
        }
        Ok(Applied::Live)
    }

    fn values(&self) -> Values {
        values_of([
            (BankSetting::Channels, Value::Text(self.list_text.clone())),
            (BankSetting::SquelchDb, Value::F64(self.squelch)),
            (BankSetting::HangS, Value::Duration(self.hang)),
            (
                BankSetting::TuneErrorHz,
                Value::Hz(self.guess.unwrap_or(TUNE_ERROR_HZ.default)),
            ),
        ])
    }
}
