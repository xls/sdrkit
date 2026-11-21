//! The `slotted_voice` stack: its settings, results, receiver graph, transmit branch, test signal and descriptor.

use super::beacon::{Beacon, Heard};
use super::v3::{Event, Receiver as V3Receiver, Transmitter as V3Transmitter};
use super::v4;
use crate::family::DEMO_CALLSIGN;
use crate::typed::{
    SlottedVoiceFields, SlottedVoiceStationEvent, SlottedVoiceStationRow, TxEvent, SLOTTED_VOICE_STATION,
    SLOTTED_VOICE_STATIONS, SLOTTED_VOICE_TX,
};
use sdrkit_algo_dsp::fft::Fft;
use sdrkit_algo_dsp::nco::Nco;
use sdrkit_algo_dsp::resample::Resampler;
use sdrkit_codec_custom::voice3::V3Rate;
use sdrkit_core::command::{Command, CommandError, CommandId, CommandSet, Commands};
use sdrkit_core::interface::{Fields, Hertz, Time};
use sdrkit_core::publish::{field, Published};
use sdrkit_core::{
    AutoGain, Choice, Complex32, ConfigError, GenConfig, Generator, LockProfile, LockState, Record, Schema, Setting,
    SettingError, SettingId, SettingSet, SignalInfo, StackConfig, StackDescriptor, StageError, Tuning, Value, Values,
};
use sdrkit_graph::{
    values_of, Applied, ChannelsFrom, Component, ComponentInfo, Decoder, DecoderComponent, Format, Io, Kept, Kind,
    Outbox, Port, RateFrom, RunContext, TagKind,
};
use std::sync::{Arc, OnceLock};

/// The stack id.
pub const ID: &str = "slotted_voice";
/// The lowest input rate.
pub const MIN_RATE: f64 = 48_000.0;
/// The speech rate of the audio output.
pub const SPEECH_RATE: f64 = 16_000.0;
/// The scale that brings the transmitter's output (gain 0.4, data with sweep and beacon) to unit mean power.
pub const UNIT_SCALE: f32 = 5.5;
/// What the stack says about its source.
pub const NOTE: &str = "Follows a private description of the signal; no public document defines it. Verified only against \
                        the test vectors of that description and its own transmitter.";

sdrkit_core::choice_enum! {
    /// The frame version that the transmitter sends.
    #[derive(Default)]
    pub enum FrameVersion {
        /// Version 3: one voice stream.
        #[default]
        V3 = ("3", "Version 3 (one voice stream)"),
        /// Version 4: four slots.
        V4 = ("4", "Version 4 (four slots)"),
    }
}

sdrkit_core::choice_enum! {
    /// The version 3 bit rate.
    #[derive(Default)]
    pub enum BitRate {
        /// 12 kbit/s at 8000 symbols a second.
        R12 = ("12", "12 kbit/s"),
        /// 16 kbit/s at 8000 symbols a second.
        #[default]
        R16 = ("16", "16 kbit/s"),
        /// 24 kbit/s at 16000 symbols a second.
        R24 = ("24", "24 kbit/s"),
        /// 32 kbit/s at 16000 symbols a second.
        R32 = ("32", "32 kbit/s"),
    }
}

impl BitRate {
    /// The codec rate.
    pub fn rate(self) -> V3Rate {
        match self {
            BitRate::R12 => V3Rate::R12,
            BitRate::R16 => V3Rate::R16,
            BitRate::R24 => V3Rate::R24,
            BitRate::R32 => V3Rate::R32,
        }
    }

    /// True for the 16000 symbols a second profile.
    pub fn fast(self) -> bool {
        matches!(self, BitRate::R24 | BitRate::R32)
    }
}

sdrkit_core::choice_enum! {
    /// The voice profile of the version 4 voice slots.
    #[derive(Default)]
    pub enum VoiceProfile {
        /// 8.0 kbit/s.
        V80 = ("v80", "8.0 kbit/s"),
        /// 6.0 kbit/s.
        #[default]
        V60 = ("v60", "6.0 kbit/s"),
        /// 4.8 kbit/s.
        V48 = ("v48", "4.8 kbit/s"),
        /// 3.2 kbit/s, QPSK.
        V32R = ("v32r", "3.2 kbit/s robust"),
    }
}

sdrkit_core::choice_enum! {
    /// The symbol rates the receiver listens for.
    #[derive(Default)]
    pub enum Listen {
        /// Both.
        #[default]
        Both = ("both", "8000 and 16000 symbols a second"),
        /// 8000 symbols a second (12 and 16 kbit/s).
        Slow = ("8000", "8000 symbols a second"),
        /// 16000 symbols a second (24 and 32 kbit/s).
        Fast = ("16000", "16000 symbols a second"),
    }
}

const TX: &str = "Transmit";

/// The station callsign.
pub const CALLSIGN: Setting<field::Text> = Setting::new("callsign", "Station callsign", "", "")
    .section(TX)
    .help("Sent in the sweep (up to 6 characters and a suffix such as /P) and in the beacon (up to 9 characters).");
/// The grid locator of the beacon.
pub const GRID: Setting<field::Text> = Setting::new("grid", "Grid locator", "", "")
    .section(TX)
    .help("Six characters such as KO26ab. Empty sends AA00aa.");
/// The latitude of the beacon.
pub const LAT: Setting<field::F64> = Setting::new("lat", "Latitude", "°", 0.0).range(-90.0, 90.0, 0.0).section(TX);
/// The longitude of the beacon.
pub const LON: Setting<field::F64> = Setting::new("lon", "Longitude", "°", 0.0).range(-180.0, 180.0, 0.0).section(TX);
/// The power value of the beacon.
pub const POWER_DBM: Setting<field::F64> =
    Setting::new("power_dbm", "Beacon power value", "dBm", 10.0).range(-40.0, 87.5, 0.5).section(TX);
/// The link quality value of the beacon.
pub const LINK: Setting<field::I64> = Setting::new("link", "Link quality value", "", 0).range(0.0, 255.0, 1.0).section(TX);
/// The beacon on or off.
pub const BEACON: Setting<field::Bool> = Setting::new("beacon", "Beacon", "", true).section(TX);
/// The beacon text.
pub const BEACON_TEXT: Setting<field::Text> = Setting::new("beacon_text", "Beacon text", "", "")
    .section(TX)
    .help("Up to 100 bytes, sent every 10 s in version 3 and in the control channel in version 4.");
/// The frame version sent.
pub const VERSION: Setting<field::Choice<FrameVersion>> =
    Setting::choice("frame_version", "Frame version", FrameVersion::V3).section(TX);
/// The version 3 bit rate.
pub const BIT_RATE: Setting<field::Choice<BitRate>> = Setting::choice("bit_rate", "Bit rate", BitRate::R16).section(TX);
/// The version 4 voice slots.
pub const VOICE_SLOTS: Setting<field::I64> =
    Setting::new("voice_slots", "Voice slots", "", 1).range(0.0, 4.0, 1.0).section(TX);
/// The version 4 voice profile.
pub const VOICE_PROFILE: Setting<field::Choice<VoiceProfile>> =
    Setting::choice("voice_profile", "Voice profile", VoiceProfile::V60).section(TX);
/// Robust data slots in version 4.
pub const ROBUST: Setting<field::Bool> = Setting::new("robust_data", "Robust data slots", "", false).section(TX);
/// The speech that `send` transmits.
pub const SPEECH_FILE: Setting<field::Text> = Setting::new("speech_file", "Speech file", "", "")
    .section(TX)
    .help("A 16-bit PCM WAV file that `send` speaks. Empty speaks a test vowel.");
/// The symbol rates searched.
pub const LISTEN: Setting<field::Choice<Listen>> = Setting::choice("listen_rate", "Symbol rates", Listen::Both);

/// The bit rate of the test signal.
pub const GEN_BIT_RATE: Setting<field::Choice<BitRate>> =
    Setting::choice("gen_bit_rate", "Test signal bit rate", BitRate::R16).section("Test signal").advanced();
/// The frame version of the test signal.
pub const GEN_VERSION: Setting<field::Choice<FrameVersion>> =
    Setting::choice("gen_frame_version", "Test signal frame version", FrameVersion::V3)
        .section("Test signal")
        .advanced();

sdrkit_core::settings! {
    /// The transmitter settings.
    pub enum TxSetting {
        /// Callsign.
        Callsign = CALLSIGN,
        /// Grid.
        Grid = GRID,
        /// Latitude.
        Lat = LAT,
        /// Longitude.
        Lon = LON,
        /// Power.
        Power = POWER_DBM,
        /// Link quality.
        Link = LINK,
        /// Beacon on.
        Beacon = BEACON,
        /// Beacon text.
        Text = BEACON_TEXT,
        /// Frame version.
        Version = VERSION,
        /// Bit rate.
        BitRate = BIT_RATE,
        /// Voice slots.
        Slots = VOICE_SLOTS,
        /// Voice profile.
        Profile = VOICE_PROFILE,
        /// Robust data.
        Robust = ROBUST,
        /// Speech file.
        Speech = SPEECH_FILE,
    }
}

sdrkit_core::settings! {
    /// The receiver settings.
    pub enum RxSetting {
        /// Symbol rates.
        Listen = LISTEN,
    }
}

sdrkit_core::settings! {
    /// The test-signal settings.
    pub enum GenSetting {
        /// Bit rate.
        BitRate = GEN_BIT_RATE,
        /// Frame version.
        Version = GEN_VERSION,
    }
}

/// The settings of the stack.
pub fn schema() -> Arc<Schema> {
    static S: OnceLock<Arc<Schema>> = OnceLock::new();
    S.get_or_init(|| Arc::new(Schema::of(&[RxSetting::SETTINGS, TxSetting::SETTINGS, GenSetting::SETTINGS].concat())))
        .clone()
}

/// One receiver core: the resampler to its notional 48 kHz and the version 3 receiver, or the version 4 receiver.
struct Core {
    /// 1 for 8000 symbols a second, 2 for 16000.
    scale: f64,
    rs: Resampler<Complex32>,
    /// The coarse carrier mixer, at the core's rate.
    nco: Nco,
    buf: Vec<Complex32>,
    rx: V3Receiver,
}

/// Points of the averaged power spectrum of the coarse carrier search.
const SPECTRUM: usize = 1024;

/// The coarse carrier of a version 3 signal before lock, from its averaged power spectrum: the band of 8 or 16 kHz that
/// stands 5 dB above the median, its width (above 11.5 kHz is 16000 symbols a second), and its left edge, where the spectrum
/// rises to a quarter of the band's mean level; the centre is 4333 or 8667 Hz above that edge (the right edge holds the
/// beacon slice).
#[derive(Default)]
struct Coarse {
    acc: Vec<f64>,
    fill: Vec<Complex32>,
    spectra: u64,
    since: f64,
}

impl Coarse {
    /// Takes samples at `rate`; once a second (after one second), the centre of the signal found, Hz, if one stands out.
    fn push(&mut self, x: &[Complex32], rate: f64) -> Option<f64> {
        if self.acc.is_empty() {
            self.acc = vec![0.0; SPECTRUM];
        }
        let mut found = None;
        for &v in x {
            self.fill.push(v);
            if self.fill.len() < SPECTRUM {
                continue;
            }
            let mut f = std::mem::take(&mut self.fill);
            Fft::forward(SPECTRUM).process(&mut f);
            // A plain mean of the first spectra, then a smoothing of 0.04: no single spectrum weighs more than its share.
            self.spectra += 1;
            let w = (1.0 / self.spectra as f64).max(0.04);
            for (a, v) in self.acc.iter_mut().zip(&f) {
                let p = f64::from(v.norm_sqr());
                *a += w * (p - *a);
            }
            self.since += SPECTRUM as f64 / rate;
            if self.since >= 1.0 {
                self.since = 0.0;
                found = self.centre(rate).or(found);
            }
        }
        found
    }

    fn centre(&self, rate: f64) -> Option<f64> {
        let n = SPECTRUM;
        let bin = rate / n as f64;
        // The spectrum from -rate/2 to +rate/2.
        let p: Vec<f64> = (0..n).map(|i| self.acc[(i + n / 2) % n]).collect();
        let hz = |i: usize| (i as f64 - (n / 2) as f64) * bin;
        let mut sorted = p.clone();
        sorted.sort_by(f64::total_cmp);
        let median = sorted[n / 2].max(1e-30);
        let best = |w: f64| -> (usize, f64) {
            let k = ((w / bin).round() as usize).clamp(1, n);
            let mut sum: f64 = p[..k].iter().sum();
            let mut top = (0, sum);
            for i in 1..=n - k {
                sum += p[i + k - 1] - p[i - 1];
                if sum > top.1 {
                    top = (i, sum);
                }
            }
            (top.0, top.1 / k as f64)
        };
        let (s8, l8) = best(8000.0);
        let (s16, l16) = best(16_000.0);
        let stands = 10f64.powf(0.5) * median;
        if l8 < stands && l16 < stands {
            return None;
        }
        // The width: the smallest run of bins that holds 90 % of the power above the median.
        let excess: Vec<f64> = p.iter().map(|v| (v - median).max(0.0)).collect();
        let total: f64 = excess.iter().sum();
        let mut width = n;
        let (mut lo, mut sum) = (0, 0.0);
        for hi in 0..n {
            sum += excess[hi];
            while sum - excess[lo] >= 0.9 * total && lo < hi {
                sum -= excess[lo];
                lo += 1;
            }
            if sum >= 0.9 * total {
                width = width.min(hi + 1 - lo);
            }
        }
        let fast = width as f64 * bin > 11_500.0;
        let (start, level, span, offset) = if fast { (s16, l16, 16_000.0, 8667.0) } else { (s8, l8, 8000.0, 4333.0) };
        // The left edge: the first bin from a quarter of the band before the window on that reaches a quarter of its level.
        let from = start.saturating_sub((span / 4.0 / bin) as usize);
        let edge = (from..n).find(|&i| p[i] >= 0.25 * level)?;
        Some(hz(edge) + offset)
    }
}

/// The `slotted_voice.rx` decoder.
pub struct SvDecoder {
    listen: Listen,
    rate: f64,
    centre: f64,
    cores: Vec<Core>,
    v4: Option<(Resampler<Complex32>, Vec<Complex32>, v4::Receiver)>,
    coarse: Coarse,
    events: Vec<Event>,
    v4_events: Vec<v4::Event>,
    heard: Vec<(Heard, f64)>,
    now: f64,
    wall: Option<sdrkit_core::Clock>,
    cleared: bool,
    started: bool,
    texts: std::collections::HashMap<String, String>,
    /// The callsign of the newest beacon.
    callsign: Option<String>,
    /// The good and bad words or slots of the last receiver that was locked.
    frames: Option<(i64, i64)>,
}

impl Default for SvDecoder {
    fn default() -> Self {
        SvDecoder {
            listen: Listen::Both,
            rate: 0.0,
            centre: 0.0,
            cores: Vec::new(),
            v4: None,
            coarse: Coarse::default(),
            events: Vec::new(),
            v4_events: Vec::new(),
            heard: Vec::new(),
            now: 0.0,
            wall: None,
            cleared: false,
            started: false,
            texts: std::collections::HashMap::new(),
            callsign: None,
            frames: None,
        }
    }
}

impl SvDecoder {
    fn build(&mut self) -> Result<(), ConfigError> {
        self.cores.clear();
        let mut scales = Vec::new();
        if self.listen != Listen::Fast {
            scales.push(1.0);
        }
        if self.listen != Listen::Slow {
            scales.push(2.0);
        }
        self.coarse = Coarse::default();
        for s in scales {
            let rs = Resampler::new(self.rate, 48_000.0 * s).map_err(|e| ConfigError::Stage(StageError::new(e)))?;
            self.cores.push(Core {
                scale: s,
                rs,
                buf: Vec::new(),
                rx: V3Receiver::new(),
                nco: Nco::new(48_000.0 * s, 0.0),
            });
        }
        let rs = Resampler::new(self.rate, 48_000.0).map_err(|e| ConfigError::Stage(StageError::new(e)))?;
        self.v4 = Some((rs, Vec::new(), v4::Receiver::new()));
        Ok(())
    }

    fn locked_core(&self) -> Option<&Core> {
        self.cores.iter().filter(|c| c.rx.report.locked).max_by_key(|c| c.rx.report.words_good)
    }

    fn time(&self) -> f64 {
        self.wall.as_ref().map_or(self.now, sdrkit_core::Clock::now)
    }
}

fn pcm_to_audio(pcm: &[i16], out: &mut Vec<f32>) {
    out.extend(pcm.iter().map(|&s| f32::from(s) / 32768.0));
}

impl Decoder for SvDecoder {
    type Input = Complex32;
    fn name(&self) -> &'static str {
        "slotted_voice.rx"
    }
    fn outputs(&self) -> Vec<Port> {
        vec![
            Port::audio("audio").rate_from(RateFrom::Fixed(SPEECH_RATE)).channels_from(ChannelsFrom::Fixed(1)),
            Port::new("events", Kind::Messages).rate_from(RateFrom::Fixed(0.0)),
        ]
    }
    fn settings(&self) -> Arc<Schema> {
        RxSetting::schema()
    }
    fn values(&self) -> Values {
        values_of([(RxSetting::Listen, Value::Choice(self.listen.index()))])
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<(), SettingError> {
        match RxSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            RxSetting::Listen => {
                self.listen = LISTEN.read(id, v)?;
                if self.rate > 0.0 {
                    let _ = self.build();
                }
            }
        }
        Ok(())
    }
    fn results(&self) -> Published {
        Published::new(SlottedVoiceFields::SHAPE.fields).with_tables(&[SLOTTED_VOICE_STATIONS.def()])
    }
    fn configure(&mut self, input: &Format) -> Result<Vec<Format>, ConfigError> {
        if input.rate < MIN_RATE {
            return Err(ConfigError::RateTooLow {
                need: MIN_RATE,
                got: input.rate,
            });
        }
        self.rate = input.rate;
        self.centre = input.centre_hz;
        self.build()?;
        let mut a = Format::audio(SPEECH_RATE, 1);
        a.bandwidth_hz = 8000.0;
        Ok(vec![a, Format::new(Kind::Messages, 0.0)])
    }
    fn retune(&mut self, input: &Format) {
        self.centre = input.centre_hz;
        let _ = self.build();
    }
    fn attach(&mut self, ctx: &RunContext) {
        self.wall = ctx.live.then(|| ctx.clock.clone());
    }
    fn process(&mut self, x: &[Complex32], first: u64, out: &mut Outbox) {
        self.started = true;
        self.now = (first + x.len() as u64) as f64 / self.rate.max(1.0);
        // Every receiver hears the input; the one with the most good words plays.
        let mut evs: Vec<Vec<Event>> = Vec::with_capacity(self.cores.len());
        // The coarse mixer holds still once any receiver decodes.
        let any_locked = self.cores.iter().any(|c| c.rx.report.locked);
        if let Some(c) = self.cores.first_mut() {
            c.buf.clear();
            c.rs.process(x, &mut c.buf);
            if !any_locked {
                // The receivers take the rest of the carrier, so the mixer moves only for a large change.
                if let Some(hz) = self.coarse.push(&c.buf, 48_000.0 * c.scale).filter(|hz| (hz + c.nco.freq()).abs() > 300.0) {
                    for c in &mut self.cores {
                        // The samples a receiver holds follow the mixer, so its search sees one carrier.
                        c.rx.carrier_step((-hz - c.nco.freq()) / c.scale);
                        c.nco.set_freq(-hz);
                    }
                }
            }
        }
        for (i, c) in self.cores.iter_mut().enumerate() {
            if i > 0 {
                c.buf.clear();
                c.rs.process(x, &mut c.buf);
            }
            c.nco.mix_in_place(&mut c.buf);
            let mut e = Vec::new();
            c.rx.process(&c.buf, &mut e);
            evs.push(e);
        }
        let best = self
            .cores
            .iter()
            .enumerate()
            .filter(|(_, c)| c.rx.report.locked)
            .max_by_key(|(_, c)| c.rx.report.words_good)
            .map(|(i, _)| i);
        for (i, e) in evs.into_iter().enumerate() {
            for ev in e {
                match ev {
                    Event::Speech(..) if Some(i) != best => {}
                    ev => self.events.push(ev),
                }
            }
        }
        let v3_locked = best.is_some();
        if let Some((rs, buf, rx)) = self.v4.as_mut() {
            buf.clear();
            rs.process(x, buf);
            rx.process(buf, &mut self.v4_events);
            if v3_locked {
                self.v4_events.retain(|e| !matches!(e, v4::Event::Speech(_)));
            }
        }
        let heard_before = self.heard.len();
        let audio = out.audio(0);
        for e in self.events.drain(..) {
            match e {
                Event::Speech(pcm, _) => pcm_to_audio(&pcm, audio),
                Event::Beacon(h) => self.heard.push((h, self.now)),
            }
        }
        for e in self.v4_events.drain(..) {
            match e {
                v4::Event::Speech(pcm) => pcm_to_audio(&pcm, audio),
                v4::Event::Beacon(b) => self.heard.push((
                    Heard {
                        beacon: b,
                        full: false,
                        at: 0,
                    },
                    self.now,
                )),
                v4::Event::Text(_) => {}
            }
        }
        let t = self.time();
        let freq = self.centre;
        for (h, _) in self.heard.iter().skip(heard_before) {
            let b = &h.beacon;
            out.messages(1).push(SLOTTED_VOICE_STATION.event(
                t,
                freq,
                &SlottedVoiceStationEvent {
                    callsign: &b.callsign,
                    locator: &b.grid,
                    position: (b.lat, b.lon),
                    power_dbm: b.power_dbm,
                    text: &String::from_utf8_lossy(&b.text),
                },
            ));
        }
    }
    fn report(&mut self, p: &mut Published) {
        if std::mem::take(&mut self.cleared) {
            p.clear_rows(&SLOTTED_VOICE_STATIONS);
        }
        let t = self.time();
        for (h, _) in self.heard.drain(..) {
            let b = &h.beacon;
            // A header without text keeps the text of the last full beacon of the station.
            let text = if b.text.is_empty() {
                self.texts.get(&b.callsign).cloned().unwrap_or_default()
            } else {
                let t = String::from_utf8_lossy(&b.text).into_owned();
                self.texts.insert(b.callsign.clone(), t.clone());
                t
            };
            SLOTTED_VOICE_STATIONS.put(
                p,
                &b.callsign,
                &SlottedVoiceStationRow {
                    callsign: &b.callsign,
                    locator: &b.grid,
                    position: (b.lat, b.lon),
                    power_dbm: b.power_dbm,
                    link: i64::from(b.link),
                    text: &text,
                    last_heard: Time(t),
                },
            );
            self.callsign = Some(b.callsign.clone());
        }
        let v4r = self.v4.as_ref().map(|v| &v.2.report);
        if let Some(c) = self.locked_core() {
            self.frames = Some((c.rx.report.words_good as i64, c.rx.report.words_bad as i64));
        } else if let Some(r) = v4r.filter(|r| r.locked) {
            self.frames = Some((r.slots_good as i64, r.slots_bad as i64));
        }
        let none = SlottedVoiceFields {
            locked: false,
            offset_hz: None,
            snr_db: None,
            frame_version: None,
            voice_kbps: None,
            symbol_rate_hz: None,
            frames_ok: self.frames.map(|f| f.0),
            frames_bad: self.frames.map(|f| f.1),
            sweep_id: None,
            sweep_db: None,
            callsign: self.callsign.as_deref(),
            note: NOTE,
        };
        let (lock, summary) = if let Some(c) = self.locked_core() {
            let r = &c.rx.report;
            let offset = -c.nco.freq() + r.carrier_hz * c.scale;
            p.put(&SlottedVoiceFields {
                locked: true,
                offset_hz: Some(Hertz(offset)),
                snr_db: r.snr_db,
                frame_version: Some(3),
                voice_kbps: r.rate.map(|r| r.frame_bits() as f64 / 20.0),
                symbol_rate_hz: Some(Hertz(8000.0 * c.scale)),
                sweep_id: r.sweep_id.as_deref(),
                sweep_db: r.sweep_db,
                ..none
            });
            (LockState::Locked, format!("Version 3 at {} symbols a second, {offset:+.0} Hz", 8000.0 * c.scale))
        } else if let Some(r) = v4r.filter(|r| r.locked) {
            p.put(&SlottedVoiceFields {
                locked: true,
                offset_hz: Some(Hertz(r.carrier_hz)),
                snr_db: r.snr_db,
                frame_version: Some(4),
                symbol_rate_hz: Some(Hertz(16_000.0)),
                ..none
            });
            (LockState::Locked, format!("Version 4, {:+.0} Hz", r.carrier_hz))
        } else {
            // The identity scanner of an unlocked receiver reads weak signals.
            let sweep_id = self.cores.iter().find_map(|c| c.rx.report.sweep_id.as_deref());
            p.put(&SlottedVoiceFields { sweep_id, ..none });
            let s = if self.started { LockState::Searching } else { LockState::Off };
            (s, "Searching for a voice signal".to_string())
        };
        p.results_mut().update_status(|s| {
            let changed = s.lock_state != lock || s.summary != summary;
            s.lock_state = lock;
            s.summary.clone_from(&summary);
            changed
        });
    }
    fn report_interval_s(&self) -> f64 {
        0.25
    }
    fn clear(&mut self) {
        for c in &mut self.cores {
            c.rx.clear();
        }
        if let Some(v) = self.v4.as_mut() {
            v.2.clear();
        }
        self.callsign = None;
        self.frames = None;
        self.cleared = true;
    }
    fn forget(&mut self, kept: &Kept) {
        for c in &mut self.cores {
            c.rx.clear();
        }
        if let Some(v) = self.v4.as_mut() {
            v.2.clear();
        }
        self.events.clear();
        self.v4_events.clear();
        self.heard.clear();
        self.callsign = None;
        self.frames = None;
        if !kept.table(SLOTTED_VOICE_STATIONS.key) {
            self.texts.clear();
            self.cleared = true;
        }
    }
}

/// The station identity and transmit choices of the sender.
#[derive(Clone, Debug, PartialEq)]
pub struct TxConfig {
    /// Callsign (empty: the demonstration callsign).
    pub callsign: String,
    /// Grid locator (empty: AA00aa).
    pub grid: String,
    /// Latitude, degrees.
    pub lat: f64,
    /// Longitude, degrees.
    pub lon: f64,
    /// Beacon power value, dBm.
    pub power_dbm: f64,
    /// Link quality value.
    pub link: u8,
    /// Beacon on.
    pub beacon: bool,
    /// Beacon text.
    pub text: String,
    /// Frame version.
    pub version: FrameVersion,
    /// Version 3 bit rate.
    pub bit_rate: BitRate,
    /// Version 4 voice slots.
    pub slots: usize,
    /// Version 4 voice profile.
    pub profile: VoiceProfile,
    /// Version 4 robust data.
    pub robust: bool,
}

impl Default for TxConfig {
    fn default() -> Self {
        TxConfig {
            callsign: String::new(),
            grid: String::new(),
            lat: 0.0,
            lon: 0.0,
            power_dbm: 10.0,
            link: 0,
            beacon: true,
            text: String::new(),
            version: FrameVersion::V3,
            bit_rate: BitRate::R16,
            slots: 1,
            profile: VoiceProfile::V60,
            robust: false,
        }
    }
}

impl TxConfig {
    /// The callsign sent.
    pub fn call(&self) -> String {
        let c = self.callsign.trim().to_ascii_uppercase();
        if c.is_empty() {
            DEMO_CALLSIGN.to_string()
        } else {
            c
        }
    }

    /// The beacon, when it is on.
    pub fn beacon(&self) -> Option<Beacon> {
        self.beacon.then(|| Beacon {
            callsign: self.call(),
            lat: self.lat,
            lon: self.lon,
            power_dbm: self.power_dbm,
            link: self.link,
            grid: if self.grid.is_empty() { "AA00aa".to_string() } else { self.grid.clone() },
            text: self.text.as_bytes().to_vec(),
        })
    }

    /// Checks the station fields.
    pub fn check(&self) -> Result<(), super::beacon::BeaconError> {
        if let Some(b) = self.beacon() {
            super::beacon::frame_bits(&b, false)?;
        }
        Ok(())
    }
}

/// A running transmitter: 16 kHz speech in, samples at the notional rate out.
pub enum Running {
    /// Version 3.
    V3(Box<V3Transmitter>),
    /// Version 4.
    V4(Box<v4::Transmitter>),
}

impl Running {
    /// The transmitter of `cfg`.
    pub fn new(cfg: &TxConfig) -> Running {
        match cfg.version {
            FrameVersion::V3 => Running::V3(Box::new(V3Transmitter::new(cfg.bit_rate.rate(), &cfg.call(), cfg.beacon()))),
            FrameVersion::V4 => Running::V4(Box::new(v4::Transmitter::new(&v4::TxOptions {
                voice: vec![
                    match cfg.profile {
                        VoiceProfile::V80 => v4::Profile::V80,
                        VoiceProfile::V60 => v4::Profile::V60,
                        VoiceProfile::V48 => v4::Profile::V48,
                        VoiceProfile::V32R => v4::Profile::V32R,
                    };
                    cfg.slots.min(4)
                ],
                robust: cfg.robust,
                callsign: cfg.call(),
                beacon: cfg.beacon(),
                text: cfg.text.clone(),
            }))),
        }
    }

    /// The notional sample rate of the output.
    pub fn rate(cfg: &TxConfig) -> f64 {
        if cfg.version == FrameVersion::V3 && cfg.bit_rate.fast() {
            96_000.0
        } else {
            48_000.0
        }
    }

    /// Takes speech and appends samples.
    pub fn push(&mut self, pcm: &[i16], out: &mut Vec<Complex32>) {
        match self {
            Running::V3(t) => t.push(pcm, out),
            Running::V4(t) => t.push(pcm, out),
        }
    }

    /// Ends the stream.
    pub fn finish(&mut self, out: &mut Vec<Complex32>) {
        match self {
            Running::V3(t) => t.finish(out),
            Running::V4(t) => t.finish(out),
        }
    }
}

/// Reads a 16-bit PCM WAV file as 16 kHz mono speech.
pub fn read_speech(path: &str) -> Option<Vec<i16>> {
    let b = std::fs::read(path).ok()?;
    if b.len() < 12 || &b[0..4] != b"RIFF" || &b[8..12] != b"WAVE" {
        return None;
    }
    let (mut i, mut rate, mut ch) = (12usize, 0u32, 0u16);
    let mut data: Option<&[u8]> = None;
    while i + 8 <= b.len() {
        let len = u32::from_le_bytes([b[i + 4], b[i + 5], b[i + 6], b[i + 7]]) as usize;
        let body = &b[i + 8..(i + 8 + len).min(b.len())];
        if &b[i..i + 4] == b"fmt " && body.len() >= 16 {
            if u16::from_le_bytes([body[0], body[1]]) != 1 || u16::from_le_bytes([body[14], body[15]]) != 16 {
                return None;
            }
            ch = u16::from_le_bytes([body[2], body[3]]);
            rate = u32::from_le_bytes([body[4], body[5], body[6], body[7]]);
        } else if &b[i..i + 4] == b"data" {
            data = Some(body);
        }
        i += 8 + len + (len & 1);
    }
    let data = data?;
    if ch == 0 || rate == 0 {
        return None;
    }
    let mono: Vec<f32> = data
        .chunks_exact(2 * ch as usize)
        .map(|f| {
            f.as_chunks::<2>().0.iter().map(|s| f32::from(i16::from_le_bytes([s[0], s[1]]))).sum::<f32>() / f32::from(ch)
        })
        .collect();
    let mut out = Vec::new();
    if rate == 16_000 {
        out = mono;
    } else {
        Resampler::<f32>::new(f64::from(rate), 16_000.0).ok()?.process(&mono, &mut out);
    }
    Some(out.iter().map(|&v| v.round().clamp(-32768.0, 32767.0) as i16).collect())
}

/// Why `send` was refused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TxError {
    /// The engine's transmit switch is off.
    Off,
    /// A transmission is under way.
    Busy,
    /// The output rate is below 48 kHz.
    Rate,
    /// The station fields cannot be sent.
    Station(String),
    /// The speech file cannot be read.
    Speech,
}

impl std::fmt::Display for TxError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TxError::Off => f.write_str("the transmitter is off"),
            TxError::Busy => f.write_str("a transmission is under way"),
            TxError::Rate => f.write_str("the transmit rate must be at least 48 kHz"),
            TxError::Station(s) => f.write_str(s),
            TxError::Speech => f.write_str("the speech file is not a 16-bit PCM WAV file"),
        }
    }
}

impl std::error::Error for TxError {}

/// Starts a transmission.
pub const SEND: Command = Command::new("send", "Send")
    .help("Speaks the speech file (or the test vowel) once with the station's settings. Transmit must be on.")
    .transmits(sdrkit_core::TxContent::Voice)
    .stopped_by("stop");
/// Ends the transmission.
pub const STOP: Command = Command::new("stop", "Stop").help("Ends the transmission at the next frame.");

sdrkit_core::commands! {
    /// The commands of the sender.
    pub enum TxCommand {
        /// Send.
        Send = SEND,
        /// Stop.
        Stop = STOP,
    }
}

/// A transmission under way: the transmitter, the speech still to speak and the resampler to the output rate.
struct Burst {
    tx: Running,
    speech: Vec<i16>,
    at: usize,
    rs: Resampler<Complex32>,
    pending: Vec<Complex32>,
    notional: Vec<Complex32>,
    ended: bool,
}

/// The sender (`slotted_voice.tx`): input `in` gives the rate and pace, output `out` the samples for `tx.sink`, output
/// `events` the `voice.tx` events.
#[derive(Default)]
pub struct SvSender {
    cfg: TxConfig,
    speech_file: String,
    rate: f64,
    transmit: bool,
    burst: Option<Burst>,
    events: Vec<sdrkit_core::Event>,
}

impl SvSender {
    fn event(&mut self, state: &str, reason: &str) {
        self.events.push(SLOTTED_VOICE_TX.event(0.0, 0.0, &TxEvent { state, reason }));
    }

    /// Starts a transmission.
    pub fn send(&mut self) -> Result<(), TxError> {
        if !self.transmit {
            return Err(TxError::Off);
        }
        if self.burst.is_some() {
            return Err(TxError::Busy);
        }
        if self.rate < MIN_RATE {
            return Err(TxError::Rate);
        }
        self.cfg.check().map_err(|e| TxError::Station(e.to_string()))?;
        let speech = if self.speech_file.is_empty() {
            super::test_speech(5 * 16_000, 1)
        } else {
            read_speech(&self.speech_file).ok_or(TxError::Speech)?
        };
        let rs = Resampler::new(Running::rate(&self.cfg), self.rate).map_err(|_| TxError::Rate)?;
        self.burst = Some(Burst {
            tx: Running::new(&self.cfg),
            speech,
            at: 0,
            rs,
            pending: Vec::new(),
            notional: Vec::new(),
            ended: false,
        });
        Ok(())
    }
}

impl Component for SvSender {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "slotted_voice.tx",
            vec![Port::iq("in")],
            vec![Port::iq("out"), Port::new("events", Kind::Messages).rate_from(RateFrom::Fixed(0.0))],
        )
        .with_settings(TxSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let f = &inputs[0];
        self.rate = f.rate;
        let dial = if f.tuned_hz > 0.0 { f.tuned_hz } else { f.centre_hz };
        Ok(vec![Format::iq(self.rate, dial, dial), Format::new(Kind::Messages, 0.0)])
    }
    fn attach(&mut self, ctx: &RunContext) {
        self.transmit = ctx.transmit;
        if !ctx.transmit && self.burst.take().is_some() {
            self.event("finished", "the transmitter is off");
        }
    }
    fn process(&mut self, io: &mut Io) {
        let room = io.input_len(0);
        let mut done = false;
        if let Some(b) = &mut self.burst {
            // 20 ms of speech at a time until the output has room's worth.
            while b.pending.len() < room && !(b.ended && b.notional.is_empty()) {
                b.notional.clear();
                if b.at < b.speech.len() {
                    let end = (b.at + 320).min(b.speech.len());
                    b.tx.push(&b.speech[b.at..end], &mut b.notional);
                    b.at = end;
                } else if !b.ended {
                    b.tx.finish(&mut b.notional);
                    b.ended = true;
                } else {
                    break;
                }
                // Unit mean power while sending.
                for v in &mut b.notional {
                    *v *= UNIT_SCALE;
                }
                b.rs.process(&b.notional, &mut b.pending);
                if b.ended {
                    b.notional.clear();
                }
            }
            let n = room.min(b.pending.len());
            let out = io.iq_out(0);
            out.extend(b.pending.drain(..n));
            if b.ended && b.pending.is_empty() {
                if let Some(last) = io.iq_out(0).len().checked_sub(1) {
                    io.output(0).tag_here(last, TagKind::BurstEnd);
                }
                done = true;
            }
        }
        if done {
            self.burst = None;
            self.event("finished", "");
        }
        io.messages_out(1).append(&mut self.events);
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        let c = &mut self.cfg;
        match TxSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            TxSetting::Callsign => c.callsign = CALLSIGN.read(id, v)?.to_owned(),
            TxSetting::Grid => c.grid = GRID.read(id, v)?.to_owned(),
            TxSetting::Lat => c.lat = LAT.read(id, v)?,
            TxSetting::Lon => c.lon = LON.read(id, v)?,
            TxSetting::Power => c.power_dbm = POWER_DBM.read(id, v)?,
            TxSetting::Link => c.link = LINK.read(id, v)?.clamp(0, 255) as u8,
            TxSetting::Beacon => c.beacon = BEACON.read(id, v)?,
            TxSetting::Text => c.text = BEACON_TEXT.read(id, v)?.chars().take(100).collect(),
            TxSetting::Version => c.version = VERSION.read(id, v)?,
            TxSetting::BitRate => c.bit_rate = BIT_RATE.read(id, v)?,
            TxSetting::Slots => c.slots = VOICE_SLOTS.read(id, v)?.clamp(0, 4) as usize,
            TxSetting::Profile => c.profile = VOICE_PROFILE.read(id, v)?,
            TxSetting::Robust => c.robust = ROBUST.read(id, v)?,
            TxSetting::Speech => self.speech_file = SPEECH_FILE.read(id, v)?.to_owned(),
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        let c = &self.cfg;
        values_of([
            (TxSetting::Callsign, Value::Text(c.callsign.clone())),
            (TxSetting::Grid, Value::Text(c.grid.clone())),
            (TxSetting::Lat, Value::F64(c.lat)),
            (TxSetting::Lon, Value::F64(c.lon)),
            (TxSetting::Power, Value::F64(c.power_dbm)),
            (TxSetting::Link, Value::I64(i64::from(c.link))),
            (TxSetting::Beacon, Value::Bool(c.beacon)),
            (TxSetting::Text, Value::Text(c.text.clone())),
            (TxSetting::Version, Value::Choice(c.version.index())),
            (TxSetting::BitRate, Value::Choice(c.bit_rate.index())),
            (TxSetting::Slots, Value::I64(c.slots as i64)),
            (TxSetting::Profile, Value::Choice(c.profile.index())),
            (TxSetting::Robust, Value::Bool(c.robust)),
            (TxSetting::Speech, Value::Text(self.speech_file.clone())),
        ])
    }
    fn commands(&self) -> &Commands {
        TxCommand::commands()
    }
    fn invoke(&mut self, id: CommandId, _args: &Values) -> Result<Option<Record>, CommandError> {
        match TxCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            TxCommand::Send => match self.send() {
                Ok(()) => self.event("started", ""),
                Err(e) => {
                    self.event("refused", &e.to_string());
                    return Err(CommandError::Failed {
                        source: StageError::new(e),
                    });
                }
            },
            TxCommand::Stop => {
                if let Some(b) = &mut self.burst {
                    b.at = b.speech.len();
                }
            }
        }
        Ok(None)
    }
}

/// The test signal: test speech without end through the transmitter, the station `TEST1` with a beacon, at unit power.
pub struct SvGenerator {
    rate: f64,
    tx: Running,
    rs: Resampler<Complex32>,
    speech: Vec<i16>,
    at: usize,
    notional: Vec<Complex32>,
    pending: Vec<Complex32>,
    bit_rate: f64,
}

impl Generator for SvGenerator {
    fn rate_hz(&self) -> f64 {
        self.rate
    }
    fn signal_info(&self) -> Option<SignalInfo> {
        Some(SignalInfo::new(10_000.0).with_bit_rate(self.bit_rate))
    }
    fn station_offset_hz(&self, _tune_offset_hz: f64) -> f64 {
        0.0
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        while self.pending.len() < out.len() {
            self.notional.clear();
            let end = self.at + 1600;
            let chunk: Vec<i16> = (self.at..end).map(|i| self.speech[i % self.speech.len()]).collect();
            self.at = end % self.speech.len();
            self.tx.push(&chunk, &mut self.notional);
            for v in &mut self.notional {
                *v *= UNIT_SCALE;
            }
            self.rs.process(&self.notional, &mut self.pending);
        }
        let n = out.len();
        for (o, v) in out.iter_mut().zip(self.pending.drain(..n)) {
            *o = v;
        }
        out.len()
    }
}

/// The station of the test signal.
pub fn test_config(version: FrameVersion, bit_rate: BitRate) -> TxConfig {
    TxConfig {
        callsign: DEMO_CALLSIGN.to_string(),
        grid: "KO24ab".into(),
        lat: 54.6,
        lon: 25.2,
        power_dbm: 10.0,
        text: format!("{DEMO_CALLSIGN} test signal"),
        version,
        bit_rate,
        ..TxConfig::default()
    }
}

/// The generator of the stack.
pub fn generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    let s = &cfg.settings;
    let tc = test_config(s.get(&GEN_VERSION), s.get(&GEN_BIT_RATE));
    let rs = Resampler::new(Running::rate(&tc), cfg.rate_hz).map_err(|e| ConfigError::Stage(StageError::new(e)))?;
    Ok(Box::new(SvGenerator {
        rate: cfg.rate_hz,
        bit_rate: if tc.version == FrameVersion::V3 { tc.bit_rate.rate().frame_bits() as f64 * 50.0 } else { 6000.0 },
        tx: Running::new(&tc),
        rs,
        speech: super::test_speech(10 * 16_000, cfg.seed),
        at: 0,
        notional: Vec::new(),
        pending: Vec::new(),
    }))
}

/// The receiver and transmitter graph: the decoder on the input, and the sender on the session's samples into the transmit
/// sink.
///
/// # Errors
///
/// The errors of the graph and of a refused setting.
pub fn graph(cfg: &StackConfig) -> Result<sdrkit_graph::StackGraph, sdrkit_graph::GraphError> {
    use sdrkit_graph::{Catalog, CommandTarget, GraphBuilder, StackGraph};
    let b = GraphBuilder::new()
        .catalog(&Catalog::of(cfg)?)
        .add("rx", DecoderComponent::new(SvDecoder::default()))
        .add("sender", SvSender::default())
        .add("tx", sdrkit_graph::components::TxSink::default())
        .input("rx")
        .input("sender.in")
        .connect("sender.out", "tx.in")
        .output("rx.audio")
        .output("rx.events")
        .output("sender.events")
        .status_from("rx")
        .clock(0.25);
    let mut g = StackGraph::of(crate::typed::shape(ID), b.build()?)
        .min_rate(MIN_RATE, "slotted_voice needs a sample rate of at least 48 kHz")
        .route(LISTEN.key(), &["rx.listen_rate"])?;
    for s in TxSetting::SETTINGS {
        let k = s.key();
        g = g.route(k, &[&format!("sender.{k}")])?;
    }
    g.command("clear", CommandTarget::every("clear"))
        .command("send", CommandTarget::of("sender", "send"))
        .command("stop", CommandTarget::of("sender", "stop"))
        .with_settings(&cfg.settings)
        .build()
}

fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn sdrkit_core::Receiver>, ConfigError> {
    Ok(Box::new(graph(cfg)?))
}

/// The stack.
pub static SLOTTED_VOICE: StackDescriptor = StackDescriptor {
    id: ID,
    name: "Slotted digital voice",
    group: crate::GROUP,
    blurb: "Digital voice in 16-QAM frames with a sweep that spells the station and a beacon of callsign, position and \
            text; version 4 carries four voice or data slots. Receive and transmit.",
    tuning: Tuning {
        min_hz: 0.1e6,
        max_hz: 6000e6,
        ..Tuning::anywhere(433.0e6, 240e3, MIN_RATE, 24e3)
    },
    submodes: &[],
    settings: schema,
    make_receiver,
    make_generator: Some(generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[],
    auto_gain: AutoGain::Radio,
    verify: Some(&crate::verify::SLOTTED_VOICE_VERIFY),
    benchmarks: Some(crate::verify::benchmarks),
};
