//! One connection to an OpenWebRX server, as the shared base drives it: the opener (upgrade, greeting, first `config`,
//! profile), the session state that the control side and the read loop share, and the read loop that decodes the audio
//! into the ring.

use crate::adpcm::{AudioDecoder, Pcm16};
use crate::conn::{self, Conn, Target, WsOptions};
use crate::proto::{self, Compression, DspParams, Hello, Msg, ServerConfig, Unread};
use crate::view::{
    run_time_id, standard, AddrState, BookmarkRow, Lists, Profile, RunTime, Written,
};
use crate::*;
use sdrkit_core::{NetError, SettingDecl, StateBlob, StateError, Value};
use sdrkit_driver_core::net::Abort;
use sdrkit_driver_core::runner::{Notices, Radio, RadioStream, Sink, StreamCtx};
use sdrkit_driver_core::ws::{op, Message, ReadError, Reader};
use sdrkit_driver_core::{Protocol, RadioSetting, RateLimits, BIAS_TEE, CENTRE, RATE, RATE_EXACT};
use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

// ---- Settings ----

/// The settings of the driver, read from the values of a radio.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Opts {
    pub delivery: Delivery,
    pub servermode: String,
    pub passband: f64,
    pub squelch: f64,
    pub secondary: Option<String>,
    pub secondary_offset: f64,
    pub dmr_slot: DmrSlot,
    pub profile: Profile,
    pub move_centre: bool,
    pub magic_key: String,
    pub user: String,
    pub password: String,
    pub rate: f64,
    pub rate_exact: bool,
    pub bias_tee: bool,
}

impl Opts {
    /// Reads the settings from `v`. The run-time settings choose from `lists`.
    pub fn from_values(v: &Values, lists: &Lists) -> Opts {
        Opts {
            delivery: v.get(&DELIVERY),
            servermode: lists.servermode(v),
            passband: v.get(&PASSBAND),
            squelch: v.get(&SQUELCH),
            secondary: lists.secondary(v),
            secondary_offset: v.get(&SECONDARY_OFFSET),
            dmr_slot: v.get(&DMR_SLOT),
            profile: lists.profile(v),
            move_centre: v.get(&MOVE_CENTRE),
            magic_key: v.get(&MAGIC_KEY).to_string(),
            user: v.get(&USER).to_string(),
            password: v.get(&PASSWORD).to_string(),
            rate: v.get(&RATE),
            rate_exact: v.get(&RATE_EXACT),
            bias_tee: v.get(&BIAS_TEE),
        }
    }

    /// The settings that take effect only at a connect.
    pub fn restart_part(&self) -> (Delivery, &str, &str) {
        (self.delivery, &self.user, &self.password)
    }

    /// The audio rate the server sends at the start: HD audio for wide FM in the `audio` delivery.
    pub fn audio_rate(&self) -> f64 {
        if self.delivery == Delivery::Audio && self.servermode == "wfm" {
            f64::from(proto::HD_OUTPUT_RATE)
        } else {
            f64::from(proto::OUTPUT_RATE)
        }
    }

    /// Refuses the standard settings this driver cannot follow.
    ///
    /// # Errors
    ///
    /// [`DriverError::Refused`] of `bias_tee` when it is on, and of `rate` for an exact rate other than the audio rate.
    pub fn check(&self) -> Result<(), DriverError> {
        if self.bias_tee {
            return Err(standard(RadioSetting::BiasTee));
        }
        if self.rate_exact && self.rate > 0.0 && (self.rate - self.audio_rate()).abs() > 0.5 {
            return Err(standard(RadioSetting::Rate));
        }
        Ok(())
    }
}

/// Where a radio connects to.
#[derive(Clone)]
pub(crate) struct At {
    pub transport: Arc<dyn Transport>,
    pub clock: Clock,
    pub target: Target,
    pub timing: Timing,
}

// ---- Session ----

struct Pending {
    f: f64,
    centre: bool,
    deadline: Instant,
}

/// Why an open or the read loop must end.
enum Halt {
    Backoff(String),
    SdrError(String),
    NotOpenWebRx,
}

/// What the read loop needs of the session for the binary messages. It is copied whenever the read loop holds the session
/// lock, so a binary message takes no lock.
#[derive(Clone, Copy, Debug, Default)]
struct Cache {
    started: bool,
    audio_comp: Compression,
    fft_comp: Compression,
    audio_rate: f64,
    band: Option<proto::Band>,
    secondary_fft: Option<(usize, f64)>,
    reset_decoders: bool,
}

/// What both the control side and the read loop see, behind one lock.
pub(crate) struct Session {
    label: String,
    conn: Arc<Conn>,
    st: Arc<AddrState>,
    notices: Notices,
    t: Timing,
    clock: Clock,
    cfg: ServerConfig,
    opts: Opts,
    /// The tuned frequency F.
    f: f64,
    sent: Option<DspParams>,
    started: Option<Instant>,
    pending: Option<Pending>,
    own_switch: bool,
    audio_rate: f64,
    logged: HashSet<String>,
    strict_json: bool,
    got_config: bool,
    configs: u64,
    dec_line: String,
    secondary_fft: Option<(usize, f64)>,
    reset_decoders: bool,
}

impl Session {
    fn lost(&self) -> DriverError {
        DriverError::Net(NetError::Closed)
    }

    fn send(&self, msg: &str) -> Result<(), DriverError> {
        self.conn.send_text(msg).map_err(|_| self.lost())
    }

    fn log(&self, line: impl AsRef<str>) {
        self.notices
            .log(format!("OpenWebRX {}: {}", self.label, line.as_ref()));
    }

    fn warn(&self, line: impl AsRef<str>) {
        self.notices
            .warn(format!("OpenWebRX {}: {}", self.label, line.as_ref()));
    }

    fn log_once(&mut self, key: &str, line: impl AsRef<str>) {
        if self.logged.insert(key.to_string()) {
            self.log(line);
        }
    }

    fn cache(&mut self) -> Cache {
        Cache {
            started: self.started.is_some(),
            audio_comp: self.cfg.audio_comp,
            fft_comp: self.cfg.fft_comp,
            audio_rate: self.audio_rate,
            band: self.cfg.band(),
            secondary_fft: self.secondary_fft,
            reset_decoders: std::mem::take(&mut self.reset_decoders),
        }
    }

    /// The width B of the `iq` delivery.
    fn b(&self) -> f64 {
        proto::iq_width(f64::from(proto::OUTPUT_RATE))
    }

    /// The demodulator frequency of the tuned frequency `f`.
    fn d_of(&self, f: f64) -> f64 {
        match self.opts.delivery {
            Delivery::Audio => f,
            Delivery::Iq => f - self.b() / 2.0,
        }
    }

    /// The tuned frequency of the demodulator frequency `d`.
    fn f_of(&self, d: f64) -> f64 {
        match self.opts.delivery {
            Delivery::Audio => d,
            Delivery::Iq => d + self.b() / 2.0,
        }
    }

    /// The target demodulator, decoder and passband.
    fn wanted(&self) -> Result<DspParams, DriverError> {
        let plan = lock(&self.st.view).plan(&self.opts)?;
        let (low, high) = match self.opts.delivery {
            Delivery::Audio => {
                proto::audio_passband(&plan.modulation, self.opts.passband, plan.bandpass)
            }
            Delivery::Iq => (0.0, self.b()),
        };
        let squelch = match self.opts.delivery {
            Delivery::Audio => self.opts.squelch,
            Delivery::Iq => proto::SQUELCH_OFF,
        };
        let centre = self.cfg.centre.unwrap_or(0.0);
        let dmr = match self.opts.dmr_slot {
            DmrSlot::One => 1,
            DmrSlot::Two => 2,
            DmrSlot::Both => 3,
        };
        Ok(DspParams {
            offset: (self.d_of(self.f) - centre).round() as i64,
            low: low as i64,
            high: high as i64,
            squelch,
            secondary_offset: (plan.secondary.is_some() && self.opts.secondary_offset > 0.0)
                .then(|| self.opts.secondary_offset.round() as i64),
            secondary: plan.secondary,
            dmr: (plan.modulation == "dmr").then_some(dmr),
            modulation: plan.modulation,
        })
    }

    fn passband(&self) -> Result<(f64, f64), DriverError> {
        let p = self.wanted()?;
        Ok((p.low as f64, p.high as f64))
    }

    /// Sends what changed since the last `dspcontrol` of this connection.
    fn push(&mut self) -> Result<(), DriverError> {
        let w = self.wanted()?;
        if let Some(m) = proto::dsp_message(self.sent.as_ref(), &w) {
            self.send(&m)?;
        }
        self.sent = Some(w);
        lock(&self.st.view).last_f = Some(self.f);
        Ok(())
    }

    fn profile_name(&self, id: &str) -> String {
        lock(&self.st.view).profile_name(id)
    }

    /// Tells the host the frequency the driver tuned, with the reason for the log.
    fn moved(&self, f: f64, why: &str) {
        self.log(why);
        self.notices.store(RadioSetting::Centre.id(), Value::Hz(f));
    }

    /// The first tune of a connection. A frequency of 0 takes the start that the server suggests.
    fn first(&mut self, wanted: f64) -> Result<String, DriverError> {
        let band = self
            .cfg
            .band()
            .ok_or(DriverError::Protocol(Protocol::MissingPart))?;
        let mut f = wanted;
        if f <= 0.0 {
            f = self.f_of(self.cfg.start().unwrap_or(band.centre));
            self.moved(
                f,
                &format!("tuned to the server's start, {} MHz", proto::fmt_mhz(f)),
            );
        }
        self.f = f;
        let (lo, hi) = self.passband()?;
        if !band.fits(self.d_of(f), lo, hi) {
            self.log(format!(
                "{} MHz is outside this OpenWebRX band ({})",
                proto::fmt_mhz(f),
                band.text()
            ));
            return Err(standard(RadioSetting::Centre));
        }
        self.push()?;
        self.send(proto::DSP_START)?;
        self.started = Some(Instant::now());
        lock(&self.st.view).iq_width = match self.opts.delivery {
            Delivery::Iq => self.b(),
            Delivery::Audio => 0.0,
        };
        Ok(match self.opts.delivery {
            Delivery::Audio => "the server demodulates: the samples are its audio, not RF".into(),
            Delivery::Iq => {
                self.log("levels from an OpenWebRX server are set by its AGC, not by the radio");
                String::new()
            }
        })
    }

    /// A live retune.
    fn retune(&mut self, f: f64) -> Result<String, DriverError> {
        let Some(band) = self.cfg.band() else {
            self.f = f;
            return Ok(String::new());
        };
        self.pending = None;
        let (lo, hi) = self.passband()?;
        let d = self.d_of(f);
        if band.fits(d, lo, hi) {
            self.f = f;
            self.push()?;
            return Ok(String::new());
        }
        let x = proto::fmt_mhz(f);
        if self.opts.profile == Profile::Follow {
            let current = self.cfg.profile();
            let best = {
                let v = lock(&self.st.view);
                v.bands
                    .iter()
                    .filter(|(id, b)| {
                        Some(id.as_str()) != current.as_deref()
                            && v.has_profile(id)
                            && b.fits(d, lo, hi)
                    })
                    .min_by(|a, b| (a.1.centre - d).abs().total_cmp(&(b.1.centre - d).abs()))
                    .map(|(id, _)| id.clone())
            };
            if let Some(id) = best {
                self.send(&proto::select_profile(&id))?;
                self.own_switch = true;
                self.pending = Some(Pending {
                    f,
                    centre: false,
                    deadline: Instant::now() + self.t.profile_wait,
                });
                return Ok(format!(
                    "switching the server to profile {} for {x} MHz",
                    self.profile_name(&id)
                ));
            }
        }
        if self.opts.move_centre && self.cfg.allow_centre != Some(false) {
            self.send(&proto::set_frequency(
                f.round() as i64,
                &self.opts.magic_key,
            ))?;
            self.own_switch = true;
            self.pending = Some(Pending {
                f,
                centre: true,
                deadline: Instant::now() + self.t.centre_wait,
            });
            return Ok(format!("asking the server to move its centre to {x} MHz"));
        }
        let p = self
            .cfg
            .profile()
            .map_or_else(|| "unknown".into(), |p| self.profile_name(&p));
        self.log(format!(
            "{x} MHz is outside this OpenWebRX band ({}, profile {p}): choose a profile that covers it",
            band.text()
        ));
        Err(standard(RadioSetting::Centre))
    }

    /// Takes new settings. On a live connection a newly named profile is selected.
    fn apply(&mut self, new: Opts) -> Result<(), DriverError> {
        let profile_changed = new.profile != self.opts.profile;
        let mut new = new;
        if self.sent.is_some() {
            new.delivery = self.opts.delivery;
            new.user = self.opts.user.clone();
            new.password = self.opts.password.clone();
        }
        self.opts = new;
        if let (true, true, Profile::Named(p)) = (
            profile_changed,
            self.sent.is_some(),
            self.opts.profile.clone(),
        ) {
            self.select(&p)?;
        }
        Ok(())
    }

    /// Sends `selectprofile`. Returns whether it was sent.
    fn select(&mut self, p: &str) -> Result<bool, DriverError> {
        if self.cfg.profile().as_deref() == Some(p) {
            return Ok(false);
        }
        if !lock(&self.st.view).has_profile(p) {
            return Err(DriverError::Refused {
                id: run_time_id(RunTime::Profile),
            });
        }
        self.send(&proto::select_profile(p))?;
        self.own_switch = true;
        Ok(true)
    }

    fn remember_band(&mut self) {
        if let (Some(p), Some(b)) = (self.cfg.profile(), self.cfg.band()) {
            lock(&self.st.view).set_band(p, b);
        }
    }

    fn rate_changed(&mut self, rate: f64) {
        self.audio_rate = rate;
        self.log(format!(
            "the server now sends audio at {} kHz",
            rate / 1000.0
        ));
        self.notices.restart();
    }

    /// The band changed under the driver.
    fn band_moved(&mut self) -> Result<(), DriverError> {
        let Some(band) = self.cfg.band() else {
            return Ok(());
        };
        let (lo, hi) = self.passband()?;
        if let Some(p) = self.pending.take() {
            if band.fits(self.d_of(p.f), lo, hi) {
                self.own_switch = false;
                self.f = p.f;
                self.push()?;
                let text = format!(
                    "the server moved to {}; tuned to {} MHz",
                    band.text(),
                    proto::fmt_mhz(p.f)
                );
                self.moved(p.f, &text);
                return Ok(());
            }
            self.warn(pending_failed(&p));
        }
        let who = if std::mem::take(&mut self.own_switch) {
            "the server moved to"
        } else {
            "another listener moved the server to"
        };
        let d = self.d_of(self.f);
        if band.fits(d, lo, hi) {
            self.log(format!("{who} {}", band.text()));
            self.push()
        } else {
            let nf = self.f_of(band.nearest(d, lo, hi));
            self.f = nf;
            self.push()?;
            let text = format!(
                "{who} {}; now tuned to {} MHz",
                band.text(),
                proto::fmt_mhz(nf)
            );
            self.warn(&text);
            self.notices.store(RadioSetting::Centre.id(), Value::Hz(nf));
            Ok(())
        }
    }

    /// The timeout of a pending profile switch or centre move.
    fn tick(&mut self) {
        if self
            .pending
            .as_ref()
            .is_some_and(|p| Instant::now() >= p.deadline)
        {
            if let Some(p) = self.pending.take() {
                self.own_switch = false;
                self.warn(pending_failed(&p));
            }
        }
    }

    fn update_readings(&self) {
        let profile = self.cfg.profile();
        let name = profile.as_deref().map(|p| self.profile_name(p));
        let mut v = lock(&self.st.view);
        v.r.profile = profile;
        v.r.profile_name = name;
        v.r.band = self.cfg.band();
        v.r.max_listeners = self.cfg.max_clients;
        v.r.fft_bins = self
            .cfg
            .fft_size
            .map(|n| i64::try_from(n).unwrap_or(i64::MAX));
        v.r.audio_adpcm = Some(self.cfg.audio_comp == Compression::Adpcm);
        v.r.waterfall_adpcm = Some(self.cfg.fft_comp == Compression::Adpcm);
        v.r.waterfall_min_db = self.cfg.waterfall_levels.map(|l| l.0);
        v.r.waterfall_max_db = self.cfg.waterfall_levels.map(|l| l.1);
    }

    fn on_config(&mut self, m: &proto::ConfigDelta) -> Result<(), DriverError> {
        let ch = self.cfg.merge(m);
        self.configs += 1;
        if ch.compression {
            self.reset_decoders = true;
        }
        if self.cfg.band().is_some() {
            if !self.got_config {
                self.got_config = true;
                self.st.reset_backoff();
            }
            if ch.band || ch.profile {
                self.remember_band();
            }
        }
        self.update_readings();
        if self.started.is_some() && (ch.band || (ch.profile && self.pending.is_some())) {
            self.band_moved()?;
        }
        Ok(())
    }

    fn decoder_text(&mut self, text: &str) {
        for c in text.chars() {
            if c == '\n' || c == '\r' {
                if !self.dec_line.is_empty() {
                    let l = std::mem::take(&mut self.dec_line);
                    self.log(format!("server decoder: {l}"));
                }
                continue;
            }
            self.dec_line.push(c);
            if self.dec_line.chars().count() >= 80 {
                let l = std::mem::take(&mut self.dec_line);
                self.log(format!("server decoder: {l}"));
            }
        }
        lock(&self.st.view).r.decoder_line = Some(self.dec_line.clone());
    }

    fn handle_text(&mut self, raw: &[u8]) -> Result<(), Halt> {
        let msg = match proto::parse(raw) {
            Ok(m) => m,
            Err(Unread::NotJson) => {
                if self.strict_json {
                    return Err(Halt::NotOpenWebRx);
                }
                self.log_once(
                    "!json",
                    "the server sent a message that is not JSON; skipped",
                );
                return Ok(());
            }
            Err(Unread::NoType) => {
                self.strict_json = false;
                self.log_once("!type", "the server sent a message without a type; skipped");
                return Ok(());
            }
            Err(Unread::Malformed(t)) => {
                self.strict_json = false;
                self.log_once(
                    &format!("bad:{t}"),
                    format!("the server sent a {t} message of another form; skipped"),
                );
                return Ok(());
            }
        };
        self.strict_json = false;
        match msg {
            Msg::Config(c) => {
                // A send that fails here leaves a dead link, which the read loop finds.
                let _ = self.on_config(&c);
            }
            Msg::SecondaryConfig { size, rate } => {
                self.secondary_fft = Some((size.max(0.0) as usize, rate));
            }
            Msg::Details(d) => {
                let text = |x: &Option<proto::Loose>| {
                    x.as_ref().map(proto::Loose::text).filter(|s| !s.is_empty())
                };
                let mut v = lock(&self.st.view);
                v.r.receiver = text(&d.receiver_name).or(v.r.receiver.take());
                v.r.location = text(&d.receiver_location).or(v.r.location.take());
                v.r.admin = text(&d.receiver_admin).or(v.r.admin.take());
                v.r.height_m = text(&d.receiver_asl)
                    .and_then(|h| h.trim().parse().ok())
                    .or(v.r.height_m);
                v.r.position = d.receiver_gps.map(|g| (g.lat, g.lon)).or(v.r.position);
            }
            Msg::Profiles(p) => {
                let list = p
                    .into_iter()
                    .map(|e| {
                        let name = e.name.unwrap_or_else(|| e.id.clone());
                        (e.id, name)
                    })
                    .collect();
                if lock(&self.st.view).set_profiles(list) {
                    self.update_readings();
                }
            }
            Msg::Features(f) => lock(&self.st.view).set_features(f),
            Msg::Modes(m) => lock(&self.st.view).set_modes(m),
            Msg::Bookmarks(b) => {
                let rows = b
                    .into_iter()
                    .take(view::MAX_LIST)
                    .map(|b| BookmarkRow {
                        name: b.name.as_ref().map(proto::Loose::text).unwrap_or_default(),
                        freq: b.frequency,
                        mode: b
                            .modulation
                            .as_ref()
                            .map(proto::Loose::text)
                            .unwrap_or_default(),
                    })
                    .collect();
                lock(&self.st.view).set_bookmarks(rows);
            }
            Msg::Dials(d) => {
                let rows = d
                    .into_iter()
                    .take(view::MAX_LIST)
                    .map(|d| {
                        let m = d.mode.as_ref().map(proto::Loose::text).unwrap_or_default();
                        (m.to_uppercase(), d.frequency)
                    })
                    .collect();
                lock(&self.st.view).set_dials(rows);
            }
            Msg::Smeter(p) => {
                lock(&self.st.view).r.smeter_db = Some(10.0 * p.max(1e-20).log10());
            }
            Msg::Clients(n) => lock(&self.st.view).r.listeners = Some(n as i64),
            Msg::Cpu(c) => lock(&self.st.view).r.cpu_pct = Some(c * 100.0),
            Msg::Temperature(c) => lock(&self.st.view).r.temperature_c = Some(c),
            Msg::Metadata(m) => {
                lock(&self.st.view).set_metadata(m.into_iter().take(view::MAX_METADATA).collect());
            }
            Msg::DecoderText(t) => self.decoder_text(&t),
            Msg::Decode(d) => {
                let now = self.clock.now();
                lock(&self.st.view).add_decode(now, d);
            }
            Msg::Log(t) => self.log(format!("server: {t}")),
            Msg::SdrError(t) => {
                lock(&self.st.view).r.server_error = Some(t.clone());
                if !self.got_config {
                    return Err(Halt::SdrError(t));
                }
                self.warn(format!("server: {t}"));
            }
            Msg::DemodError(t) => {
                self.warn(format!("server demodulator: {t}"));
                let mut v = lock(&self.st.view);
                v.r.server_error = Some(t.clone());
                let named: Vec<String> = v
                    .modes
                    .iter()
                    .flatten()
                    .map(|m| m.id.clone())
                    .filter(|id| t.split(|c: char| !c.is_alphanumeric()).any(|w| w == id))
                    .collect();
                if !named.is_empty() {
                    v.mark_unavailable(named);
                }
            }
            Msg::Chat { name, text } => self.log(format!("chat {name}: {text}")),
            Msg::Backoff { reason } => return Err(Halt::Backoff(reason)),
            Msg::Unused(t) => self.log_once(
                &format!("type:{t}"),
                format!("the server sent a message of type {t}, not used"),
            ),
        }
        Ok(())
    }

    fn capabilities(&self) -> Capabilities {
        let (mut lo, mut hi) = (0.0, 0.0);
        if let Some(b) = self.cfg.band() {
            (lo, hi) = (b.lo(), b.hi());
            if self.opts.profile == Profile::Follow {
                for b in lock(&self.st.view).bands.values() {
                    lo = f64::min(lo, b.lo());
                    hi = f64::max(hi, b.hi());
                }
            }
        }
        let r = self.audio_rate;
        Capabilities {
            name: format!("OpenWebRX {} (network)", self.label),
            rates: RateLimits {
                min_hz: r,
                max_hz: r,
                ranges: vec![(r, r)],
                steady_hz: None,
            },
            min_freq_hz: lo,
            max_freq_hz: hi,
            gain_db: None,
            bandwidth_hz: None,
            has_agc: false,
            transmit: None,
            has_bias_tee: false,
            realtime: true,
            common_rates_hz: Vec::new(),
            common_bandwidths_hz: Vec::new(),
            gain_steps_db: Vec::new(),
        }
    }
}

fn pending_failed(p: &Pending) -> String {
    let mut s = format!("the server did not move to {} MHz", proto::fmt_mhz(p.f));
    if p.centre {
        s.push_str(" (it may not allow listeners to move the centre, or the magic key is wrong)");
    }
    s
}

// ---- Binary messages ----

/// The message types of the server.
mod kind {
    /// A waterfall line of the band.
    pub const FFT: u8 = 0x01;
    /// Narrow audio.
    pub const AUDIO: u8 = 0x02;
    /// A line of the decoder spectrum.
    pub const SECONDARY_FFT: u8 = 0x03;
    /// HD audio.
    pub const HD_AUDIO: u8 = 0x04;
}

/// The decoding state of one connection: the audio decoders, buffers that are used again, and counters.
pub(crate) struct Proc {
    st: Arc<AddrState>,
    narrow: AudioDecoder,
    hd: AudioDecoder,
    pcm16: Pcm16,
    pcm: Vec<i16>,
    fft: Vec<f32>,
    cache: Cache,
    logged_types: [bool; 256],
    lost_unlogged: u64,
    lost_logged_at: Option<Instant>,
    win_start: Instant,
    win_bytes: u64,
    win_audio: u64,
    win_lines: u64,
    last_audio: Option<Instant>,
    last_line: Option<Instant>,
    said_no_audio: bool,
    /// The audio rate changed: the next tick starts a new throughput window.
    new_window: bool,
}

impl Proc {
    fn new(st: Arc<AddrState>) -> Proc {
        Proc {
            st,
            narrow: AudioDecoder::default(),
            hd: AudioDecoder::default(),
            pcm16: Pcm16::default(),
            pcm: Vec::with_capacity(8192),
            fft: Vec::with_capacity(8192),
            cache: Cache::default(),
            logged_types: [false; 256],
            lost_unlogged: 0,
            lost_logged_at: None,
            win_start: Instant::now(),
            win_bytes: 0,
            win_audio: 0,
            win_lines: 0,
            last_audio: None,
            last_line: None,
            said_no_audio: false,
            new_window: false,
        }
    }

    /// Takes what the binary messages need from the session.
    fn refresh(&mut self, s: &mut Session) {
        self.cache = s.cache();
        if std::mem::take(&mut self.cache.reset_decoders) {
            self.narrow = AudioDecoder::default();
            self.hd = AudioDecoder::default();
            self.pcm16 = Pcm16::default();
        }
    }

    fn handle(&mut self, s: &Mutex<Session>, sink: Option<&mut Sink>, b: &[u8]) {
        let Some((&ty, p)) = b.split_first() else {
            return;
        };
        if p.is_empty() {
            return;
        }
        match ty {
            kind::FFT | kind::SECONDARY_FFT => {
                match self.cache.fft_comp {
                    Compression::Adpcm => crate::adpcm::decode_fft_adpcm(p, &mut self.fft),
                    Compression::None => crate::adpcm::decode_fft_f32(p, &mut self.fft),
                }
                let n = self.fft.len().max(1) as f64;
                let mut v = lock(&self.st.view);
                if ty == kind::FFT {
                    self.win_lines += 1;
                    self.last_line = Some(Instant::now());
                    if let Some(band) = self.cache.band {
                        let step = band.rate / n;
                        v.waterfall.push(&self.fft, band.lo() + step / 2.0, step);
                    }
                } else {
                    let span = self.cache.secondary_fft.map_or(n, |(_, r)| r);
                    v.decoder_fft.push(&self.fft, -span / 2.0, span / n);
                }
            }
            kind::AUDIO | kind::HD_AUDIO => self.audio(s, sink, ty, p),
            t => {
                if !std::mem::replace(&mut self.logged_types[usize::from(t)], true) {
                    lock(s).log(format!(
                        "the server sent a binary message of type {t:#04x}, not used"
                    ));
                }
            }
        }
    }

    fn audio(&mut self, s: &Mutex<Session>, sink: Option<&mut Sink>, ty: u8, p: &[u8]) {
        let rate = f64::from(if ty == kind::HD_AUDIO {
            proto::HD_OUTPUT_RATE
        } else {
            proto::OUTPUT_RATE
        });
        if (self.cache.audio_rate - rate).abs() > 0.5 {
            let mut g = lock(s);
            g.rate_changed(rate);
            self.refresh(&mut g);
            self.new_window = true;
        }
        let write = self.cache.started.then_some(sink).flatten();
        let n = match self.cache.audio_comp {
            Compression::Adpcm => {
                self.pcm.clear();
                let dec = if ty == kind::HD_AUDIO {
                    &mut self.hd
                } else {
                    &mut self.narrow
                };
                let lost = dec.decode_adpcm(p, &mut self.pcm);
                if lost > 0 {
                    self.resync(s, lost);
                }
                if let Some(sink) = write {
                    let pcm = &self.pcm;
                    sink.write_with(pcm.len(), |dst, from| {
                        for (d, x) in dst.as_chunks_mut::<2>().0.iter_mut().zip(&pcm[from..]) {
                            let [lo, hi] = x.to_le_bytes();
                            d[0].write(lo);
                            d[1].write(hi);
                        }
                    });
                }
                self.pcm.len()
            }
            Compression::None => {
                let (joined, body) = self.pcm16.split(p);
                if let Some(sink) = write {
                    if let Some(j) = joined {
                        sink.write(&j);
                    }
                    sink.write(body);
                }
                body.len() / 2 + usize::from(joined.is_some())
            }
        };
        if !self.cache.started || n == 0 {
            return;
        }
        self.win_audio += n as u64;
        self.last_audio = Some(Instant::now());
    }

    fn resync(&mut self, s: &Mutex<Session>, lost: u64) {
        let g = lock(s);
        g.st.add_lost(lost);
        self.lost_unlogged += lost;
        let quiet = self
            .lost_logged_at
            .is_some_and(|t| t.elapsed() < Duration::from_secs(10));
        if !quiet {
            self.lost_logged_at = Some(Instant::now());
            g.log(format!(
                "the audio stream lost its sync: about {} samples lost",
                std::mem::take(&mut self.lost_unlogged)
            ));
        }
    }

    /// Once a tick: the no-audio rule and the throughput window.
    fn tick(&mut self, s: &mut Session, bytes_in: u64) {
        let now = Instant::now();
        if let Some(st) = s.started {
            let since_audio = self.last_audio.map_or(now - st, |t| now - t);
            let lines = self
                .last_line
                .is_some_and(|t| now - t < Duration::from_secs(2));
            if lines && since_audio >= s.t.no_audio && !self.said_no_audio {
                self.said_no_audio = true;
                s.log(
                    "the server sends a waterfall but no audio (its demodulator may have failed)",
                );
            }
        }
        if std::mem::take(&mut self.new_window) {
            (
                self.win_start,
                self.win_bytes,
                self.win_audio,
                self.win_lines,
            ) = (now, bytes_in, 0, 0);
            return;
        }
        let secs = (now - self.win_start).as_secs_f64();
        if secs >= s.t.window.as_secs_f64() {
            let dec = if s.audio_rate > f64::from(proto::OUTPUT_RATE) {
                &self.hd
            } else {
                &self.narrow
            };
            let mut v = lock(&s.st.view);
            v.r.kbit_s = Some(bytes_in.saturating_sub(self.win_bytes) as f64 * 8.0 / secs / 1e3);
            v.r.audio_sps = Some(self.win_audio as f64 / secs);
            v.r.lines_s = Some(self.win_lines as f64 / secs);
            v.r.bytes_in = Some(i64::try_from(bytes_in).unwrap_or(i64::MAX));
            v.r.audio_sync = match s.cfg.audio_comp {
                Compression::None => None,
                Compression::Adpcm => dec.synced(),
            };
            (
                self.win_start,
                self.win_bytes,
                self.win_audio,
                self.win_lines,
            ) = (now, bytes_in, 0, 0);
        }
    }
}

// ---- The opener ----

struct Opening<'a> {
    s: Session,
    reader: Reader,
    proc: Proc,
    abort: &'a Arc<Abort>,
}

impl Opening<'_> {
    /// Reads messages until `done` holds (`Ok(true)`) or the deadline passes (`Ok(false)`).
    fn pump(
        &mut self,
        deadline: Instant,
        done: impl Fn(&Session) -> bool,
    ) -> Result<bool, DriverError> {
        loop {
            if done(&self.s) {
                return Ok(true);
            }
            if self.abort.fired() {
                return Err(DriverError::Net(NetError::Aborted));
            }
            if Instant::now() >= deadline {
                return Ok(false);
            }
            match self.reader.poll_message() {
                Ok(None) => {}
                Ok(Some(Message::Text(t))) => {
                    if let Err(h) = self.s.handle_text(t) {
                        return Err(self.halt(h));
                    }
                }
                Ok(Some(Message::Binary(_))) => {}
                Err(e) => return Err(read_error(e, &self.s)),
            }
        }
    }

    fn halt(&mut self, h: Halt) -> DriverError {
        self.s.conn.send_close();
        self.s.conn.shutdown();
        match h {
            Halt::Backoff(reason) => {
                let w = self.s.st.back_off(&self.s.t);
                self.s.warn(format!(
                    "the server is busy: {reason}; waiting {:.1} s",
                    w.as_secs_f64()
                ));
                DriverError::Busy
            }
            Halt::SdrError(t) => {
                self.s.warn(format!("server: {t}"));
                DriverError::Busy
            }
            Halt::NotOpenWebRx => DriverError::Protocol(Protocol::NotThisRadio),
        }
    }
}

/// The error of a read that ended the connection, with a line for the log. When the connection closed, it marks the
/// connection as closed by the server.
fn read_error(e: ReadError, s: &Session) -> DriverError {
    let line = match e {
        ReadError::Bad(_) => "the server sent a bad WebSocket frame".to_string(),
        ReadError::TooBig => "the server sent a message over 4 MiB".into(),
        ReadError::Closed => {
            s.conn.peer_closed.store(true, Ordering::Release);
            // The shared reader answers the close frame of the server, so a close that went out without one of the client
            // means that the server closed first.
            if s.conn.close_sent() {
                "the server closed the connection".into()
            } else {
                "lost the connection".into()
            }
        }
        _ => "lost the connection".into(),
    };
    s.log(line);
    DriverError::from(e)
}

/// The opener: connect, upgrade, greeting, first configuration and a named profile. The base then calls `configure` for the
/// first tune.
pub(crate) fn connect(
    at: &At,
    st: &Arc<AddrState>,
    values: &Values,
    abort: &Arc<Abort>,
    notices: &Notices,
) -> Result<OwrxRadio, DriverError> {
    let t = &at.timing;
    if !st.wait_backoff(abort) {
        return Err(DriverError::Net(NetError::Aborted));
    }
    let opts = Opts::from_values(values, &lock(&st.view).lists());
    let wo = WsOptions {
        transport: &at.transport,
        user: &opts.user,
        password: &opts.password,
        timing: t,
    };
    let (conn, reader, used) = conn::open(&at.target, &wo, abort).map_err(|e| {
        if e.back_off {
            let w = st.back_off(t);
            notices.warn(format!(
                "OpenWebRX {}: the server is busy; waiting {:.1} s",
                at.target.label_at(at.target.port()),
                w.as_secs_f64()
            ));
        }
        e.error
    })?;
    let label = used.label_at(used.port());
    conn.send_text(proto::GREETING)
        .map_err(|_| DriverError::Net(NetError::Closed))?;
    conn.send_text(&proto::connection_properties())
        .map_err(|_| DriverError::Net(NetError::Closed))?;
    let audio_rate = opts.audio_rate();
    let s = Session {
        label: label.clone(),
        conn: conn.clone(),
        st: st.clone(),
        notices: notices.clone(),
        t: t.clone(),
        clock: at.clock.clone(),
        cfg: ServerConfig::default(),
        opts,
        f: values.get(&CENTRE),
        sent: None,
        started: None,
        pending: None,
        own_switch: false,
        audio_rate,
        logged: HashSet::new(),
        strict_json: false,
        got_config: false,
        configs: 0,
        dec_line: String::new(),
        secondary_fft: None,
        reset_decoders: false,
    };
    let mut o = Opening {
        s,
        reader,
        proc: Proc::new(st.clone()),
        abort,
    };
    if let Err(e) = open_steps(&mut o) {
        o.s.conn.shutdown();
        return Err(e);
    }
    Ok(OwrxRadio {
        label,
        conn,
        st: st.clone(),
        s: Arc::new(Mutex::new(o.s)),
        reader: Some(o.reader),
        proc: Some(o.proc),
        reading: Arc::new(AtomicBool::new(false)),
        closed: false,
        close_wait: t.close_wait,
        tick: t.tick,
        written: Written::default(),
    })
}

fn open_steps(o: &mut Opening) -> Result<(), DriverError> {
    let t = o.s.t.clone();
    // The greeting.
    let deadline = Instant::now() + t.greeting;
    loop {
        if o.abort.fired() {
            return Err(DriverError::Net(NetError::Aborted));
        }
        match o.reader.poll_message() {
            Ok(Some(Message::Text(x))) => {
                let text = String::from_utf8_lossy(x).to_string();
                let hello = proto::parse_hello(&text);
                let known = hello.known();
                match hello {
                    Hello::Classic(v) => {
                        o.s.log(
                            "this server is older than OpenWebRX 0.18 and speaks the text protocol",
                        );
                        return Err(DriverError::Version {
                            got: v.map_or(0, proto::version_number),
                            need: proto::FIRST_JSON_VERSION,
                        });
                    }
                    Hello::NotOpenWebRx => {
                        return Err(DriverError::Protocol(Protocol::NotThisRadio))
                    }
                    Hello::Json {
                        name, version_text, ..
                    } => {
                        let shown = match name.as_deref() {
                            Some("openwebrx") | None => "OpenWebRX".to_string(),
                            Some(n) => n.to_string(),
                        };
                        let server = match &version_text {
                            Some(v) => format!("{shown} {v}"),
                            None => shown,
                        };
                        if !known {
                            o.s.strict_json = true;
                            o.s.log(format!(
                                "the server calls itself {} (version {}): treated as OpenWebRX",
                                name.as_deref().unwrap_or("nothing"),
                                version_text.as_deref().unwrap_or("unknown")
                            ));
                        }
                        lock(&o.s.st.view).r.server = Some(server);
                        break;
                    }
                }
            }
            Ok(Some(Message::Binary(_))) => {}
            Ok(None) => {
                if Instant::now() >= deadline {
                    o.s.log("the server accepted the WebSocket but did not answer the OpenWebRX greeting");
                    return Err(DriverError::Net(NetError::TimedOut));
                }
            }
            Err(e) => return Err(read_error(e, &o.s)),
        }
    }
    // The first usable configuration.
    let deadline = Instant::now() + t.config;
    if !o.pump(deadline, |s| s.got_config)? {
        o.s.log(format!(
            "the server greeted but sent no receiver configuration within {} s",
            t.config.as_secs_f64()
        ));
        return Err(DriverError::Net(NetError::TimedOut));
    }
    // A named profile.
    if let Profile::Named(p) = o.s.opts.profile.clone() {
        if lock(&o.s.st.view).profiles.is_empty() {
            o.pump(Instant::now() + t.profiles_wait, |s| {
                !lock(&s.st.view).profiles.is_empty()
            })?;
        }
        let before = o.s.configs;
        if o.s.select(&p)? {
            let deadline = Instant::now() + t.profile_wait;
            if !o.pump(deadline, |s| s.configs > before)? {
                o.s.log(format!("the server did not switch to profile {p}"));
            }
        }
        o.s.own_switch = false;
    }
    Ok(())
}

// ---- The radio ----

/// The control side of one connection.
pub struct OwrxRadio {
    label: String,
    conn: Arc<Conn>,
    st: Arc<AddrState>,
    s: Arc<Mutex<Session>>,
    reader: Option<Reader>,
    proc: Option<Proc>,
    reading: Arc<AtomicBool>,
    closed: bool,
    close_wait: Duration,
    tick: Duration,
    written: Written,
}

impl Radio for OwrxRadio {
    type Stream = OwrxStream;

    fn readings(&mut self, results: &mut Results) {
        self.st.write(results, &mut self.written);
    }

    fn settings(&self) -> Vec<SettingDecl> {
        lock(&self.st.view).lists().settings()
    }

    fn save_state(&self) -> Option<StateBlob> {
        lock(&self.st.view).save()
    }

    fn restore_state(&mut self, state: &StateBlob) -> Result<(), StateError> {
        lock(&self.st.view).restore(state)
    }

    fn configure(&mut self, values: &Values, _live: bool) -> Result<String, DriverError> {
        let mut s = lock(&self.s);
        let lists = lock(&self.st.view).lists();
        s.apply(Opts::from_values(values, &lists))?;
        let mut f = values.get(&CENTRE);
        if s.sent.is_none() {
            let v = lock(&self.st.view);
            if v.refused_f == Some(f) {
                if let Some(l) = v.last_f {
                    f = l;
                }
            }
            drop(v);
            return s.first(f);
        }
        s.push()?;
        if f > 0.0 && (f - s.f).abs() > 0.5 {
            let r = s.retune(f);
            lock(&self.st.view).refused_f = r.is_err().then_some(f);
            r
        } else {
            Ok(String::new())
        }
    }

    fn check(&self, values: &Values) -> Result<(), DriverError> {
        let v = lock(&self.st.view);
        v.check(&Opts::from_values(values, &v.lists()))
    }

    fn stream(&mut self, abort: &Arc<Abort>) -> Result<OwrxStream, DriverError> {
        let lost = || DriverError::Net(NetError::Closed);
        let reader = self.reader.take().ok_or_else(lost)?;
        let mut proc = self.proc.take().ok_or_else(lost)?;
        proc.refresh(&mut lock(&self.s));
        self.conn.set_tick(self.tick);
        self.conn.register(abort);
        Ok(OwrxStream {
            label: self.label.clone(),
            conn: self.conn.clone(),
            s: self.s.clone(),
            reader,
            proc,
            reading: self.reading.clone(),
        })
    }

    fn close(&mut self) {
        if self.closed {
            return;
        }
        self.closed = true;
        let c = &self.conn;
        if !c.peer_closed.load(Ordering::Acquire) {
            c.send_close();
            let deadline = Instant::now() + self.close_wait;
            if let Some(r) = self.reader.as_mut() {
                // No read loop: wait for the close of the server here.
                while Instant::now() < deadline {
                    if r.poll_message().is_err() {
                        break;
                    }
                }
            } else {
                while self.reading.load(Ordering::Acquire)
                    && !c.peer_closed.load(Ordering::Acquire)
                    && Instant::now() < deadline
                {
                    std::thread::sleep(Duration::from_millis(5));
                }
            }
        }
        c.shutdown();
    }

    fn rate_hz(&self) -> f64 {
        lock(&self.s).audio_rate
    }

    fn centre_hz(&self) -> f64 {
        let s = lock(&self.s);
        s.d_of(s.f)
    }

    fn capabilities(&self) -> Capabilities {
        lock(&self.s).capabilities()
    }

    fn sample_type(&self) -> SampleType {
        SampleType::I16
    }
}

impl Drop for OwrxRadio {
    fn drop(&mut self) {
        self.conn.shutdown();
    }
}

// ---- The read loop ----

/// The read loop of one connection.
pub struct OwrxStream {
    label: String,
    conn: Arc<Conn>,
    s: Arc<Mutex<Session>>,
    reader: Reader,
    proc: Proc,
    reading: Arc<AtomicBool>,
}

struct Reading(Arc<AtomicBool>);
impl Drop for Reading {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

impl RadioStream for OwrxStream {
    fn run(&mut self, sink: &mut Sink, ctx: &StreamCtx) -> Result<(), DriverError> {
        self.reading.store(true, Ordering::Release);
        let _reading = Reading(self.reading.clone());
        let t = lock(&self.s).t.clone();
        let mut last_rx = Instant::now();
        let mut last_frame = Instant::now();
        let mut last_tick = Instant::now();
        let mut seen = self.reader.bytes_in;
        loop {
            let now = Instant::now();
            if self.reader.bytes_in != seen {
                seen = self.reader.bytes_in;
                last_rx = now;
            }
            let stopping = ctx.stopping() || self.conn.close_sent();
            if !stopping && now - last_tick >= t.tick {
                last_tick = now;
                {
                    let mut s = lock(&self.s);
                    s.tick();
                    self.proc.tick(&mut s, self.reader.bytes_in);
                    self.proc.refresh(&mut s);
                }
                if self.conn.idle() >= t.keepalive {
                    let _ = self.conn.send(op::PING, b"sdrkit");
                }
                if now - last_rx >= t.stall || now - last_frame >= t.stall * 2 {
                    ctx.notices.log(format!(
                        "OpenWebRX {}: no data for {} s",
                        self.label,
                        t.stall.as_secs_f64()
                    ));
                    return Err(DriverError::Net(NetError::TimedOut));
                }
            }
            match self.reader.poll_message() {
                Ok(None) => {}
                Ok(Some(Message::Text(x))) => {
                    last_frame = now;
                    let mut s = lock(&self.s);
                    let r = s.handle_text(x);
                    self.proc.refresh(&mut s);
                    if let Err(Halt::Backoff(reason)) = r {
                        let w = s.st.back_off(&s.t);
                        drop(s);
                        self.conn.send_close();
                        self.conn.shutdown();
                        if ctx.stopping() {
                            return Ok(());
                        }
                        ctx.notices.warn(format!(
                            "OpenWebRX {}: the server is busy: {reason}; waiting {:.1} s before the next connect",
                            self.label,
                            w.as_secs_f64()
                        ));
                        return Err(DriverError::Busy);
                    }
                }
                Ok(Some(Message::Binary(b))) => {
                    last_frame = now;
                    let sink = (!stopping).then_some(&mut *sink);
                    self.proc.handle(&self.s, sink, b);
                }
                Err(e) => {
                    // `stopping` was taken before this read, so a close that the reader sent in answer to the close of
                    // the server does not count as one of the client.
                    let closed = e == ReadError::Closed;
                    let err = read_error(e, &lock(&self.s));
                    if closed {
                        self.conn.shutdown();
                    }
                    if stopping || ctx.stopping() {
                        return Ok(());
                    }
                    return Err(err);
                }
            }
        }
    }
}
