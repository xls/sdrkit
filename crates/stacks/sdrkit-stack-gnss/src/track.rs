//! The tracking channel: early, prompt and late correlators over each code period, the carrier loop (a Costas phase lock with a
//! frequency-locked pull-in), the code loop aided by the carrier, C/N0 by the moments method, the lock checks, the message
//! layer, and the satellite time of every epoch.

use crate::acq::AcqHit;
use crate::codes::SignalKind;
use crate::frontend::{BandBuffer, SinCos, LUT_BITS};
use crate::msg::{Layer, MsgEvent, NavUnit, Sync};
use crate::system::SatId;
use sdrkit_algo_dsp::loopfilter::LoopFilter;
use sdrkit_algo_modem::carrier::{CarrierConfig, CarrierSync, Costas};
use sdrkit_algo_modem::level::{cn0_dbhz, M2m4};
use sdrkit_algo_modem::lock::HalfRateFalseLock;
use sdrkit_core::Complex32;
use std::collections::VecDeque;

/// The state of a tracking channel.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum ChannelState {
    PullIn,
    Locked,
    BitSync,
    FrameSync,
    Ephemeris,
}

impl sdrkit_core::Choice for ChannelState {
    const CHOICES: &'static [sdrkit_core::ChoiceDecl] = &[
        sdrkit_core::ChoiceDecl::new("pull_in", "Pull-in"),
        sdrkit_core::ChoiceDecl::new("locked", "Locked"),
        sdrkit_core::ChoiceDecl::new("bit_sync", "Bit sync"),
        sdrkit_core::ChoiceDecl::new("frame_sync", "Frame sync"),
        sdrkit_core::ChoiceDecl::new("ephemeris", "Ephemeris"),
    ];
    fn index(self) -> u16 {
        self as u16
    }
    fn from_index(index: u16) -> Option<Self> {
        [
            ChannelState::PullIn,
            ChannelState::Locked,
            ChannelState::BitSync,
            ChannelState::FrameSync,
            ChannelState::Ephemeris,
        ]
        .get(usize::from(index))
        .copied()
    }
}

/// What a channel reports to the receiver.
#[derive(Clone, Debug, PartialEq)]
pub enum ChanEvent {
    Unit {
        unit: NavUnit,
        start_epoch: u64,
        time: Option<f64>,
    },
    Bad,
    Locked,
    Log(String),
    /// The channel is gone, with the reason; `confirmed` false when it never locked.
    Lost {
        reason: String,
        confirmed: bool,
    },
}

/// The shared loop filter with the gains (ζ = 0.707, ωn = Bn / 0.53): `kp = 2 ζ ωn`, `ki = ωn² t`
/// for noise bandwidth `bn` (Hz) and update period `t` (s), so the output is in the error's units per second. The integrator starts
/// at `integrator`.
fn loop_filter(bn: f64, t: f64, integrator: f64) -> LoopFilter {
    let wn = bn / 0.53;
    let mut f = LoopFilter::from_gains(2.0 * 0.707 * wn, wn * wn * t);
    f.set_integrator(integrator);
    f
}

/// The carrier loop's noise bandwidth after lock, Hz.
const PLL_BN_HZ: f64 = 15.0;

/// The frequency-locked pull-in's noise bandwidth per epoch.
const FLL_BN_T: f64 = 0.032;

/// The carrier loop of a channel with epochs of `t` seconds.
fn carrier_sync(t: f64) -> CarrierSync<Costas<2>> {
    let mut cfg = CarrierConfig::new(PLL_BN_HZ * t);
    cfg.fll_bn_t = Some(FLL_BN_T);
    cfg.max_freq = 0.25;
    cfg.lock_updates = 100.0;
    cfg.lock_on = 0.2;
    cfg.lock_off = 0.1;
    CarrierSync::new(Costas::<2>, cfg)
        .expect("the carrier loop settings hold for every code period")
}

/// Epochs the C/N0 estimate needs before it is ready.
const CN0_FIRST: u64 = 50;

/// The channel's C/N0: the shared exponentially weighted M2M4 estimator on the prompts, smoothed for display.
#[derive(Clone, Debug)]
pub struct Cn0 {
    m2m4: M2m4,
    pub value: f64,
    ready: bool,
}

impl Default for Cn0 {
    fn default() -> Self {
        Cn0 {
            m2m4: M2m4::psk().with_memory(500).unwrap_or_else(|_| M2m4::psk()),
            value: 0.0,
            ready: false,
        }
    }
}

impl Cn0 {
    /// Takes the prompt of an epoch of `t` seconds.
    pub fn update(&mut self, p: Complex32, t: f64) {
        self.m2m4.push(p);
        if self.m2m4.count() < CN0_FIRST {
            return;
        }
        let snr = self.m2m4.estimate().map_or(1e-3, |e| e.ratio().max(1e-3));
        let Some(c) = cn0_dbhz(snr, t) else {
            return;
        };
        if !self.ready {
            self.value = c;
            self.ready = true;
        } else {
            self.value += 0.01 * (c - self.value);
        }
    }
    pub fn ready(&self) -> bool {
        self.ready
    }
}

/// The half-rate false-lock test: after 100 epochs of lock, over 500 epochs.
fn false_lock_test() -> HalfRateFalseLock {
    HalfRateFalseLock::new(100, 500).expect("the false-lock window is not empty")
}

/// One tracking channel.
pub struct Channel {
    pub sat: SatId,
    pub kind: SignalKind,
    /// The nominal carrier of the signal (GLONASS: of its frequency channel).
    pub carrier_hz: f64,
    fs: f64,
    /// Replica units (chips, or half chips for BOC) as ±1.
    code: Vec<f32>,
    units_per_chip: f64,
    code_len: f64,
    /// Band index (fractional) where the next code period starts.
    pub next_start: f64,
    /// Carrier frequency in the band (Hz) and phase (cycles) at sample `phase_at`.
    pub carr_freq: f64,
    carr_phase: f64,
    phase_at: u64,
    /// The code loop's rate correction (chips/s) and the hint from other channels (fractional).
    pub code_corr: f64,
    pub code_hint: f64,
    dll: LoopFilter,
    carrier: CarrierSync<Costas<2>>,
    start_freq: f64,
    pub state: ChannelState,
    pull_epochs: u32,
    pub locked_epochs: u64,
    low_li: u32,
    low_cn0: u32,
    false_lock: HalfRateFalseLock,
    pub false_locks: u32,
    pub cn0: Cn0,
    pub cn0_history: VecDeque<f32>,
    /// Epochs counted since the start.
    pub epoch: u64,
    /// (epoch, band position of its start) of the last 256 epochs.
    pub epoch_pos: VecDeque<(u64, f64)>,
    /// The satellite time of an epoch, once known.
    pub time: Option<(u64, f64)>,
    pub layer: Layer,
    pub scatter: VecDeque<[f32; 2]>,
    pub good: u64,
    pub bad: u64,
    pub lock_time_s: Option<f64>,
    pub started_s: f64,
    /// Whether the channel was carrier locked for longer than 0.5 s at some point.
    pub was_locked: bool,
    y_re: Vec<f32>,
    y_im: Vec<f32>,
    rep: Vec<f32>,
    msg: Vec<MsgEvent>,
    lost: bool,
    glo_k: i8,
}

impl Channel {
    /// A channel from a confirmed hit, starting a few code periods before `newest` (the band's end).
    pub fn start(
        sat: SatId,
        kind: SignalKind,
        carrier_hz: f64,
        chips: &[u8],
        hit: &AcqHit,
        newest: u64,
        code_hint: f64,
        code_corr: f64,
        now_s: f64,
    ) -> Channel {
        let fs = kind.band_rate();
        let boc = kind == SignalKind::E1b;
        let code: Vec<f32> = if boc {
            chips
                .iter()
                .flat_map(|&c| {
                    let v = if c == 0 { 1.0 } else { -1.0 };
                    [v, -v]
                })
                .collect()
        } else {
            chips
                .iter()
                .map(|&c| if c == 0 { 1.0 } else { -1.0 })
                .collect()
        };
        let code_len = chips.len() as f64;
        let chip_rate =
            kind.chip_rate() * (1.0 + hit.doppler_hz / carrier_hz + code_hint) + code_corr;
        let per = code_len / chip_rate * fs;
        let back = if boc { 2.0 } else { 3.0 };
        let first = hit.seg_start as f64 + hit.code_phase;
        let target = newest as f64 - (back + 1.0) * per;
        let m = ((target - first) / per).floor().max(0.0);
        let next_start = first + m * per;
        Channel {
            sat,
            kind,
            carrier_hz,
            fs,
            code,
            units_per_chip: if boc { 2.0 } else { 1.0 },
            code_len,
            next_start,
            carr_freq: hit.doppler_hz,
            carr_phase: 0.0,
            phase_at: next_start.ceil() as u64,
            code_corr,
            code_hint,
            dll: loop_filter(15.0, kind.period_s(), code_corr),
            carrier: carrier_sync(kind.period_s()),
            start_freq: hit.doppler_hz,
            state: ChannelState::PullIn,
            pull_epochs: 0,
            locked_epochs: 0,
            low_li: 0,
            low_cn0: 0,
            false_lock: false_lock_test(),
            false_locks: 0,
            cn0: Cn0::default(),
            cn0_history: VecDeque::with_capacity(60),
            epoch: 0,
            epoch_pos: VecDeque::with_capacity(257),
            time: None,
            layer: Layer::for_sat(sat),
            scatter: VecDeque::with_capacity(401),
            good: 0,
            bad: 0,
            lock_time_s: None,
            started_s: now_s,
            was_locked: false,
            y_re: Vec::new(),
            y_im: Vec::new(),
            rep: Vec::new(),
            msg: Vec::new(),
            lost: false,
            glo_k: sat.glo_k().unwrap_or(0),
        }
    }

    /// The code period, s.
    pub fn period(&self) -> f64 {
        self.kind.period_s()
    }

    /// The code frequency (chips/s), carrier aided.
    pub fn code_freq(&self) -> f64 {
        self.kind.chip_rate() * (1.0 + self.carr_freq / self.carrier_hz + self.code_hint)
            + self.code_corr
    }

    /// Whether the carrier is locked.
    pub fn locked(&self) -> bool {
        self.state >= ChannelState::Locked
    }

    /// The band index the channel needs next (for trimming).
    pub fn needs_from(&self) -> u64 {
        (self.next_start.floor() as u64).saturating_sub(8)
    }

    /// Runs every epoch the band holds. Returns `false` once the channel is lost (its events say why).
    pub fn run(
        &mut self,
        band: &BandBuffer,
        lut: &SinCos,
        now_s: f64,
        events: &mut Vec<ChanEvent>,
    ) -> bool {
        if self.lost {
            return false;
        }
        let d = self.kind.el_offset();
        loop {
            let rate = self.code_freq() / self.fs; // chips per sample
            let per = self.code_len / rate;
            let n0 = self.next_start.ceil() as u64;
            let n_end = (self.next_start + per).ceil() as u64;
            let n = (n_end - n0) as usize;
            if n0 < band.start + d as u64 {
                self.lose(events, "samples already gone (processing too slow)");
                return false;
            }
            let Some(x) = band.slice(n0, n + d) else {
                return true;
            };
            // Carrier phase at n0.
            let ph =
                self.carr_phase + self.carr_freq * (n0 as f64 - self.phase_at as f64) / self.fs;
            self.carr_phase = ph.rem_euclid(1.0);
            self.phase_at = n0;
            // Wipe the carrier.
            self.y_re.resize(n, 0.0);
            self.y_im.resize(n, 0.0);
            let mut acc = (self.carr_phase * 4_294_967_296.0) as u32;
            let inc = ((self.carr_freq / self.fs).rem_euclid(1.0) * 4_294_967_296.0).round() as u32;
            let shift = 32 - LUT_BITS;
            for i in 0..n {
                let k = (acc >> shift) as usize;
                let (c, s) = (lut.cos[k], lut.sin[k]);
                let v = x[i];
                self.y_re[i] = v.re * c + v.im * s;
                self.y_im[i] = v.im * c - v.re * s;
                acc = acc.wrapping_add(inc);
            }
            // The replica over [n0 - d, n0 + n + d): r[j] = code at the code phase of sample n0 - d + j.
            let urate = rate * self.units_per_chip;
            let ulen = self.code.len();
            let phi0 = (n0 as f64 - self.next_start) * urate - d as f64 * urate;
            self.rep.resize(n + 2 * d, 0.0);
            let mut pos = (phi0.rem_euclid(ulen as f64) * 4_294_967_296.0) as u64;
            let uinc = (urate * 4_294_967_296.0) as u64;
            let lim = (ulen as u64) << 32;
            for r in self.rep.iter_mut() {
                *r = self.code[(pos >> 32) as usize];
                pos += uinc;
                if pos >= lim {
                    pos -= lim;
                }
            }
            let (mut ei, mut eq, mut pi, mut pq, mut li, mut lq) =
                (0f32, 0f32, 0f32, 0f32, 0f32, 0f32);
            {
                let early = &self.rep[2 * d..2 * d + n];
                let prompt = &self.rep[d..d + n];
                let late = &self.rep[..n];
                for i in 0..n {
                    let (a, b) = (self.y_re[i], self.y_im[i]);
                    ei += a * early[i];
                    eq += b * early[i];
                    pi += a * prompt[i];
                    pq += b * prompt[i];
                    li += a * late[i];
                    lq += b * late[i];
                }
            }
            let norm = 1.0 / (n as f32).sqrt();
            let p = Complex32::new(pi * norm, pq * norm);
            let e_mag = ((ei * ei + eq * eq) as f64).sqrt();
            let l_mag = ((li * li + lq * lq) as f64).sqrt();
            // Record the epoch.
            let e = self.epoch;
            self.epoch_pos.push_back((e, self.next_start));
            if self.epoch_pos.len() > 256 {
                self.epoch_pos.pop_front();
            }
            self.epoch += 1;
            let t = self.period();
            self.cn0.update(p, t);
            if (e + 1).is_multiple_of((1.0 / t).round() as u64) {
                self.cn0_history.push_back(self.cn0.value as f32);
                if self.cn0_history.len() > 60 {
                    self.cn0_history.pop_front();
                }
            }
            // Code loop.
            let dll_err = if e_mag + l_mag > 0.0 {
                0.5 * (e_mag - l_mag) / (e_mag + l_mag)
            } else {
                0.0
            };
            let bn = if self.epoch < 400 {
                15.0
            } else if self.epoch < 2000 {
                5.0
            } else {
                1.2
            };
            // A steady error is absorbed by the integrator (the code-rate correction).
            self.dll = loop_filter(bn, t, self.dll.integrator());
            let out = self.dll.update(dll_err);
            self.code_corr = out;
            // Advance to the next period; the loop output moves the code over it.
            let rate2 = self.code_freq() / self.fs;
            self.next_start += self.code_len / rate2;
            // Carrier loop.
            self.carrier_loop(p, t, events, now_s);
            if self.lost {
                return false;
            }
        }
    }

    fn carrier_loop(&mut self, p: Complex32, t: f64, events: &mut Vec<ChanEvent>, now_s: f64) {
        // The loop turns the prompt, and its frequency moves into the carrier wipe-off of the next epoch.
        let y = self.carrier.push(p);
        self.carr_freq += self.carrier.take_freq() / t;
        if self.state == ChannelState::PullIn {
            self.pull_epochs += 1;
            if self.pull_epochs > 300
                && self.carrier.locked()
                && self.cn0.ready()
                && self.cn0.value > 24.0
            {
                self.state = ChannelState::Locked;
                self.locked_epochs = 0;
                self.false_lock.reset();
                if self.lock_time_s.is_none() {
                    self.lock_time_s = Some(now_s);
                }
                events.push(ChanEvent::Locked);
            } else if self.pull_epochs >= 1500 && self.cn0.value < 24.0 {
                self.lose(events, "too weak to lock");
            } else if self.pull_epochs >= 6000 {
                self.lose(events, "carrier did not settle");
            } else if (self.carr_freq - self.start_freq).abs() > 3000.0 {
                self.lose(events, "carrier ran off");
            }
            return;
        }
        self.locked_epochs += 1;
        if self.locked_epochs as f64 * t > 0.5 {
            self.was_locked = true;
        }
        self.scatter.push_back([y.re, y.im]);
        if self.scatter.len() > 400 {
            self.scatter.pop_front();
        }
        // False lock half the epoch rate away, where a symbol lasts two or more epochs.
        if self.layer.long_symbols() && self.false_lock.push(y) == Some(true) {
            self.false_lock_step(events);
        }
        if self.locked_epochs > 300 {
            if !self.carrier.locked() {
                self.low_li += 1;
                if self.low_li > 500 {
                    self.low_li = 0;
                    self.state = ChannelState::PullIn;
                    self.pull_epochs = 0;
                    self.layer.reset();
                    self.time = None;
                    events.push(ChanEvent::Log(
                        "phase lock dropped (lock indicator low for 0.5 s), pulling in again"
                            .into(),
                    ));
                    return;
                }
            } else {
                self.low_li = 0;
            }
        }
        if self.locked_epochs > 1000 {
            if self.cn0.value < 17.0 {
                self.low_cn0 += 1;
                if self.low_cn0 > 3000 {
                    self.lose(events, "signal faded");
                    return;
                }
            } else {
                self.low_cn0 = 0;
            }
        }
        if self.locked_epochs >= 100 {
            let e = self.epoch - 1;
            self.msg.clear();
            let mut msg = std::mem::take(&mut self.msg);
            self.layer.epoch(e, y, &mut msg);
            for m in msg.drain(..) {
                self.on_msg(m, events);
            }
            self.msg = msg;
            let sync = self.layer.sync();
            self.state = match sync {
                Sync::Frame if self.state == ChannelState::Ephemeris => ChannelState::Ephemeris,
                Sync::Frame => ChannelState::FrameSync,
                Sync::Bit => ChannelState::BitSync,
                Sync::None => ChannelState::Locked,
            };
        }
    }

    fn false_lock_step(&mut self, events: &mut Vec<ChanEvent>) {
        let half = 0.5 / self.period();
        let dir = if self.start_freq > self.carr_freq {
            1.0
        } else {
            -1.0
        };
        self.carr_freq += dir * half;
        self.locked_epochs = 0;
        self.false_lock.reset();
        self.false_locks += 1;
        self.layer.reset();
        self.time = None;
        events.push(ChanEvent::Log(format!(
            "false lock {:.0} Hz away: carrier moved",
            half
        )));
    }

    fn on_msg(&mut self, m: MsgEvent, events: &mut Vec<ChanEvent>) {
        match m {
            MsgEvent::Unit {
                unit,
                start_epoch,
                time,
            } => {
                if let Some(t) = time {
                    if let Some(pred) = self.time_of_epoch(start_epoch) {
                        let modulus = if matches!(unit, NavUnit::Glo { .. }) {
                            86_400.0
                        } else {
                            604_800.0
                        };
                        let mut d = (t - pred).rem_euclid(modulus);
                        if d > modulus / 2.0 {
                            d -= modulus;
                        }
                        if d.abs() > 2e-4 {
                            self.time = None;
                            self.layer.reset();
                            self.bad += 1;
                            events.push(ChanEvent::Bad);
                            events.push(ChanEvent::Log(
                                "a new time disagreed with the one held: synchronisation dropped"
                                    .into(),
                            ));
                            return;
                        }
                    }
                    self.time = Some((start_epoch, t));
                }
                self.good += 1;
                events.push(ChanEvent::Unit {
                    unit,
                    start_epoch,
                    time,
                });
            }
            MsgEvent::Bad => {
                self.bad += 1;
                events.push(ChanEvent::Bad);
            }
            MsgEvent::CarrierStep(hz) => {
                self.carr_freq += hz;
            }
            MsgEvent::FalseLock => self.false_lock_step(events),
            MsgEvent::LostFrame => {
                self.time = None;
            }
            MsgEvent::Log(l) => events.push(ChanEvent::Log(l)),
        }
    }

    fn lose(&mut self, events: &mut Vec<ChanEvent>, why: &str) {
        self.lost = true;
        events.push(ChanEvent::Lost {
            reason: why.into(),
            confirmed: self.lock_time_s.is_some(),
        });
    }

    /// Marks the channel lost from outside (a discontinuity, a dropped band).
    pub fn drop_with(&mut self, events: &mut Vec<ChanEvent>, why: &str) {
        if !self.lost {
            self.lose(events, why);
        }
    }

    /// The satellite time of epoch `k` from the held time.
    pub fn time_of_epoch(&self, k: u64) -> Option<f64> {
        let (k0, t0) = self.time?;
        Some(t0 + (k as f64 - k0 as f64) * self.period())
    }

    /// The transmit time (satellite clock) at band position `pos`, by interpolation between the epochs around it.
    pub fn transmit_time_at(&self, pos: f64) -> Option<f64> {
        self.time?;
        let v = &self.epoch_pos;
        if v.len() < 2 {
            return None;
        }
        for w in 0..v.len() - 1 {
            let (k0, p0) = v[w];
            let (_, p1) = v[w + 1];
            if p0 <= pos && pos < p1 {
                let t0 = self.time_of_epoch(k0)?;
                return Some(t0 + (pos - p0) / (p1 - p0) * self.period());
            }
        }
        None
    }

    /// The newest band position with a recorded epoch.
    pub fn newest_pos(&self) -> f64 {
        self.epoch_pos.back().map(|e| e.1).unwrap_or(0.0)
    }

    /// The code phase in chips at the newest sample (for the channel table).
    pub fn code_chips(&self, at: f64) -> f64 {
        let rate = self.code_freq() / self.fs;
        ((at - self.next_start) * rate).rem_euclid(self.code_len)
    }

    /// The steady code-rate error the code loop absorbed, as a fraction of the chip rate.
    pub fn code_rate_err(&self) -> f64 {
        self.dll.integrator() / self.kind.chip_rate()
    }

    /// The GLONASS frequency channel of the channel.
    pub fn glo_k(&self) -> i8 {
        self.glo_k
    }
    /// Sets the GLONASS frequency channel.
    pub fn set_glo_k(&mut self, k: i8) {
        self.glo_k = k;
    }

    /// The lock indicator: the averaged cos 2 phi of the carrier loop.
    pub fn lock_indicator(&self) -> f64 {
        self.carrier.lock_metric() as f64
    }
}
