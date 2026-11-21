//! Time signals (`time`): DCF77, MSF, WWVB, JJY 40 and 60, BPC, WWV and WWVH, BPM, HLA and RWM through the receiver, and CHU and the
//! BPM coded minute as codecs and test signals. The graph is a front end that makes a 1 kHz level stream, a finder and slicer
//! of the seconds, and the minute decoder that keeps the time and compares the computer clock.

pub mod codes;

use crate::calendar;
use crate::clock::{ClockChoice, ClockMode, LiveClock, CLOCK_MODE, CLOCK_ORIGIN};
use crate::family::{lock_state_of, status, wall, write_series, REPORT_S};
use codes::{EncodeOptions, Refusal, Station, Sym, TimeFrame};
use sdrkit_algo_dsp::meter::power_to_db;
use sdrkit_algo_dsp::nco::Nco;
use sdrkit_algo_dsp::resample::Resampler;
use sdrkit_core::publish::keys::Family;
use crate::typed::{
    TimeFields, TimeMinuteEvent, TimeMinuteRow, TIME_MINUTE, TIME_MINUTES, TIME_SERIES,
};
use sdrkit_core::interface::Time;
use sdrkit_core::publish::{field, Published};
use sdrkit_core::verify::{CaseSetting, CheckLine, VerifyCase, VerifyHooks, VerifyOptions};
use sdrkit_core::{
    Choice, Clock, CommandError, CommandId, CommandSet, Commands, Event, Record,
    SeriesId, SettingId, SettingSet, StateBlob, StateError, Status,
};
use sdrkit_core::{
    Complex32, ConfigError, GenConfig, Generator, Hz, Receiver, Results, ResultsDecl, Setting,
    SettingError, SignalInfo, StackConfig, Value, Values,
};
use sdrkit_graph::{
    values_of, Applied, Catalog, Component, ComponentInfo, Format, GraphBuilder, GraphError, Io,
    Item, Kind, Port, RateFrom, RunContext, StackGraph, TagKind, Timed,
};
use sdrkit_util_geo::Position;
use sdrkit_util_objects::commands as objcmd;
use sdrkit_util_objects::shaped::{log_rows, RowOut, Shaped};
use sdrkit_util_objects::{Format as StateFormat, LogRing, Seen, Shift, Tables};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::f64::consts::TAU;
use std::fmt::Write as _;

sdrkit_core::choice_enum! {
    /// The station a receiver decodes: a fixed one, or the one of the dial frequency.
    pub enum StationPick {
        /// The station of the dial frequency.
        Auto = ("auto", "Automatic (from the dial)"),
        /// DCF77.
        Dcf77 = ("dcf77", "DCF77"),
        /// MSF.
        Msf = ("msf", "MSF"),
        /// WWVB.
        Wwvb = ("wwvb", "WWVB"),
        /// JJY 40 kHz.
        Jjy40 = ("jjy40", "JJY 40 kHz"),
        /// JJY 60 kHz.
        Jjy60 = ("jjy60", "JJY 60 kHz"),
        /// BPC.
        Bpc = ("bpc", "BPC"),
        /// WWV or WWVH.
        Wwv = ("wwv", "WWV / WWVH"),
        /// CHU.
        Chu = ("chu", "CHU"),
        /// BPM.
        Bpm = ("bpm", "BPM"),
        /// The BPM coded minute.
        Bpmc = ("bpmc", "BPM coded"),
        /// HLA.
        Hla = ("hla", "HLA"),
        /// RWM.
        Rwm = ("rwm", "RWM"),
    }
}

impl StationPick {
    /// The fixed station, or `None` for the station of the dial.
    pub fn station(self) -> Option<Station> {
        Station::from_index(self.index().checked_sub(1)?)
    }
}

sdrkit_core::choice_enum! {
    /// The station that the automatic choice takes at 60 kHz.
    pub enum Sixty {
        /// MSF.
        Msf = ("msf", "MSF"),
        /// WWVB.
        Wwvb = ("wwvb", "WWVB"),
        /// JJY 60 kHz.
        Jjy60 = ("jjy60", "JJY 60 kHz"),
    }
}

impl Sixty {
    /// The station.
    pub fn station(self) -> Station {
        match self {
            Sixty::Msf => Station::Msf,
            Sixty::Wwvb => Station::Wwvb,
            Sixty::Jjy60 => Station::Jjy60,
        }
    }
}

/// The station to decode.
pub const STATION_SETTING: Setting<field::Choice<StationPick>> =
    Setting::choice("station", "Station", StationPick::Auto);
/// The station that the automatic choice takes at 60 kHz.
pub const SIXTY_KHZ: Setting<field::Choice<Sixty>> =
    Setting::choice("sixty_khz", "60 kHz is", Sixty::Msf);
/// The samples are a receiver's demodulated audio.
pub const AUDIO_INPUT: Setting<field::Bool> =
    Setting::new("audio_input", "Demodulated audio recording", "", false);
/// The carrier tone of an LF audio recording.
pub const AUDIO_TONE_HZ: Setting<field::Hz> = Setting::new(
    "audio_tone_hz",
    "Carrier tone of an LF recording",
    "Hz",
    1000.0,
)
.range(100.0, 20000.0, 1.0);
/// How slow the recording's clock runs.
pub const CLOCK_SLOW_PCT: Setting<field::F64> =
    Setting::new("clock_slow_pct", "Recording clock slow by", "%", 0.0).range(0.0, 10.0, 0.01);
/// Play the signal.
pub const LISTEN: Setting<field::Bool> = Setting::new("listen", "Listen", "", false);
/// The year of the stations that send none, 0 for the year of the host clock.
pub const YEAR_HINT: Setting<field::I64> =
    Setting::new("year_hint", "Year hint", "", 0).range(0.0, 3000.0, 1.0);
/// The UTC of the test signal's first sample, 0 for the host clock.
pub const GEN_START_UTC: Setting<field::F64> = Setting::new(
    "gen_start_utc",
    "Test signal: start (UTC, 0 = host clock)",
    "s",
    0.0,
)
.range(0.0, 1e11, 1.0)
.section("Test signal")
.advanced();

sdrkit_core::settings! {
    /// The settings of the time-signal stack.
    pub enum TimeSetting {
        /// The station to decode.
        Station = STATION_SETTING,
        /// The station of 60 kHz.
        SixtyKhz = SIXTY_KHZ,
        /// The samples are demodulated audio.
        AudioInput = AUDIO_INPUT,
        /// The carrier tone of an LF audio recording.
        AudioToneHz = AUDIO_TONE_HZ,
        /// How slow the recording's clock runs.
        ClockSlowPct = CLOCK_SLOW_PCT,
        /// Play the signal.
        Listen = LISTEN,
        /// What the received time is compared with.
        ClockMode = CLOCK_MODE,
        /// The UTC of sample 0 for the fixed clock.
        ClockOrigin = CLOCK_ORIGIN,
        /// The year of the stations that send none.
        YearHint = YEAR_HINT,
        /// The UTC of the test signal's first sample.
        GenStartUtc = GEN_START_UTC,
    }
}

/// Which station a receiver decodes: a fixed one, or the one of the dial frequency.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StationChoice {
    /// The setting.
    pub pick: StationPick,
    /// The station the automatic choice takes at 60 kHz.
    pub sixty: Sixty,
}

impl Default for StationChoice {
    fn default() -> Self {
        StationChoice {
            pick: STATION_SETTING.default,
            sixty: SIXTY_KHZ.default,
        }
    }
}

impl StationChoice {
    /// The station for a dial frequency, or `None` when the automatic choice finds none there.
    pub fn resolve(&self, tuned_hz: f64) -> Option<Station> {
        match self.pick.station() {
            Some(st) => Some(st),
            None => Station::auto(tuned_hz, self.sixty.station()),
        }
    }
}

/// What the level stream measures for a station.
fn tone_of(st: Station) -> Option<f64> {
    match st {
        Station::Wwv | Station::Hla => Some(100.0),
        Station::Bpm | Station::Bpmc => Some(1000.0),
        Station::Chu => Some(1000.0),
        _ => None,
    }
}

/// Whether the second's edge rises (pulses start on the second) or falls (the carrier drops on the second).
fn rising(st: Station) -> bool {
    !matches!(
        st,
        Station::Dcf77 | Station::Msf | Station::Wwvb | Station::Bpc
    )
}

/// The "always high" and "always low" regions (ms after the second).
fn regions(st: Station) -> ((usize, usize), (usize, usize)) {
    match st {
        Station::Dcf77 => ((300, 950), (20, 90)),
        Station::Msf => ((600, 950), (15, 85)),
        Station::Wwvb => ((850, 985), (20, 180)),
        Station::Bpc => ((450, 950), (20, 90)),
        Station::Jjy40 | Station::Jjy60 => ((20, 180), (850, 985)),
        Station::Rwm => ((12, 28), (400, 950)),
        Station::Wwv | Station::Hla => ((20, 180), (850, 985)),
        Station::Bpm | Station::Bpmc | Station::Chu => ((20, 90), (400, 950)),
    }
}

/// The level stream (1 kHz) with the station's facts.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Levels {
    /// Signal time of the first value.
    pub t0: f64,
    /// The values.
    pub v: Vec<f32>,
    /// Carrier level, dBFS.
    pub carrier_dbfs: f64,
    /// 1000 Hz and 1200 Hz tick levels (WWV / WWVH).
    pub ticks: (f64, f64),
    /// The station decoded, or `None` when there is none at the dial frequency.
    pub station: Option<Station>,
}

impl Item for Levels {
    const KIND: &'static str = "time.level";
}

/// The front end (`time.front`): the station mixed to 0 Hz, resampled to 8 kHz, and the carrier envelope (or the envelope of the
/// AM audio's tone) at 1 kHz.
pub struct Front {
    rate: f64,
    choice: StationChoice,
    station: Option<Station>,
    audio_input: bool,
    tone_hz: f64,
    slow: f64,
    osc: Nco,
    resampler: Option<Resampler<Complex32>>,
    delay_s: f64,
    start: Option<u64>,
    mixed: Vec<Complex32>,
    y8: Vec<Complex32>,
    box_buf: VecDeque<f32>,
    box_sum: f64,
    am_dc: f64,
    corr: VecDeque<Complex32>,
    corr_sum: Complex32,
    tone_osc: Nco,
    n8: u64,
    carrier: f64,
    tick1000: Nco,
    tick1200: Nco,
    t1: (Complex32, Complex32),
    tick_acc: (f64, f64),
    listen: bool,
    beep: f64,
    format: Option<Format>,
}

impl Default for Front {
    fn default() -> Self {
        Front {
            rate: 0.0,
            choice: StationChoice::default(),
            station: None,
            audio_input: false,
            tone_hz: 1000.0,
            slow: 0.0,
            osc: Nco::new(1.0, 0.0),
            resampler: None,
            delay_s: 0.0,
            start: None,
            mixed: Vec::new(),
            y8: Vec::new(),
            box_buf: VecDeque::new(),
            box_sum: 0.0,
            am_dc: 0.0,
            corr: VecDeque::new(),
            corr_sum: Complex32::new(0.0, 0.0),
            tone_osc: Nco::new(W, 100.0),
            n8: 0,
            carrier: 0.0,
            tick1000: Nco::new(W, 1000.0),
            tick1200: Nco::new(W, 1200.0),
            t1: (Complex32::new(0.0, 0.0), Complex32::new(0.0, 0.0)),
            tick_acc: (0.0, 0.0),
            listen: false,
            beep: 0.0,
            format: None,
        }
    }
}

const W: f64 = 8000.0;

impl Front {
    fn setup(&mut self, f: &Format) {
        self.format = Some(f.clone());
        self.station = self.choice.resolve(f.tuned_hz);
        let st = self.station.unwrap_or(Station::Dcf77);
        self.rate = f.rate * (1.0 - self.slow / 100.0);
        let off = if self.audio_input {
            if st.info().lf {
                self.tone_hz
            } else {
                0.0
            }
        } else {
            f.signal_offset_hz()
        };
        self.osc = Nco::new(self.rate, -off);
        self.resampler = Resampler::new(self.rate, W).ok();
        self.delay_s = self
            .resampler
            .as_ref()
            .map_or(0.0, |r| r.delay() / self.rate);
        self.start = None;
        let win = if tone_of(st) == Some(100.0) { 160 } else { 80 };
        self.box_buf = VecDeque::from(vec![0.0; 160]);
        self.box_sum = 0.0;
        self.corr = VecDeque::from(vec![Complex32::new(0.0, 0.0); win]);
        self.corr_sum = Complex32::new(0.0, 0.0);
        self.tone_osc = Nco::new(W, -tone_of(st).unwrap_or(100.0));
        self.n8 = 0;
    }
}

sdrkit_core::settings! {
    /// The settings of [`Front`].
    pub enum FrontSetting {
        /// The station to decode.
        Station = STATION_SETTING,
        /// The station of 60 kHz.
        SixtyKhz = SIXTY_KHZ,
        /// The samples are demodulated audio.
        AudioInput = AUDIO_INPUT,
        /// The carrier tone of an LF audio recording.
        AudioToneHz = AUDIO_TONE_HZ,
        /// How slow the recording's clock runs.
        ClockSlowPct = CLOCK_SLOW_PCT,
        /// Play the signal.
        Listen = LISTEN,
    }
}

impl Component for Front {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "time.front",
            vec![Port::iq("in")],
            vec![
                Port::typed::<Levels>("levels").rate_from(RateFrom::Fixed(0.0)),
                Port::audio("audio").rate_from(RateFrom::Fixed(W)),
            ],
        )
        .with_settings(FrontSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.setup(&inputs[0]);
        Ok(vec![
            Format::new(Kind::typed::<Levels>(), 0.0),
            Format::new(Kind::Audio, W),
        ])
    }
    fn process(&mut self, io: &mut Io) {
        let first = io.input(0).first();
        for t in io.input(0).tags() {
            if let TagKind::Retuned {
                centre_hz,
                tuned_hz,
            } = t.kind
            {
                if let Some(mut f) = self.format.clone() {
                    f.centre_hz = centre_hz;
                    f.tuned_hz = tuned_hz;
                    self.setup(&f);
                }
            }
        }
        let start = *self.start.get_or_insert(first);
        let x = io.iq(0);
        self.mixed.clear();
        let gain = if self.audio_input { 2.0 } else { 1.0 };
        for &s in x {
            let s = if self.audio_input {
                Complex32::new(s.re * gain, 0.0)
            } else {
                s
            };
            self.mixed.push(s * self.osc.next_sample());
        }
        self.y8.clear();
        if let Some(r) = self.resampler.as_mut() {
            r.process(&self.mixed, &mut self.y8);
        }
        let mut lv = Levels {
            station: self.station,
            ..Levels::default()
        };
        let audio = &mut io.audio_out(1).samples;
        let st = self.station.unwrap_or(Station::Dcf77);
        let lf = st.info().lf;
        let tone = tone_of(st);
        let mut cpow = 0.0;
        let mut cn = 0usize;
        let base = start as f64 / self.rate.max(1.0);
        for k in 0..self.y8.len() {
            let y = self.y8[k];
            self.n8 += 1;
            let mag = y.norm();
            cpow += y.norm_sqr() as f64;
            cn += 1;
            let value = if lf || tone.is_none() {
                // Carrier magnitude over 20 ms.
                self.box_buf.push_back(mag);
                self.box_sum += mag as f64;
                self.box_sum -= self.box_buf.pop_front().unwrap_or(0.0) as f64;
                (self.box_sum / 160.0) as f32
            } else {
                // AM audio: the envelope minus the carrier, then the tone's envelope over the window.
                self.am_dc += 0.001 * (mag as f64 - self.am_dc);
                let a = mag - self.am_dc as f32;
                let c = self.tone_osc.next_sample() * a;
                self.corr.push_back(c);
                self.corr_sum += c;
                self.corr_sum -= self.corr.pop_front().unwrap_or_default();
                // WWV and WWVH ticks.
                let p1 = self.tick1000.next_sample() * a;
                let p2 = self.tick1200.next_sample() * a;
                self.t1.0 = self.t1.0 * 0.95 + p1;
                self.t1.1 = self.t1.1 * 0.95 + p2;
                self.tick_acc.0 = self.tick_acc.0.max(self.t1.0.norm_sqr() as f64);
                self.tick_acc.1 = self.tick_acc.1.max(self.t1.1.norm_sqr() as f64);
                (self.corr_sum.norm() / self.corr.len() as f32) * 2.0
            };
            if self.listen {
                let v = if lf {
                    self.beep += 800.0 / W;
                    value * (TAU * self.beep).sin() as f32
                } else {
                    mag - self.am_dc as f32
                };
                audio.push(v);
            }
            if self.n8.is_multiple_of(8) {
                if lv.v.is_empty() {
                    // The 8 kHz sample n stands for signal time n / W less the resampler's delay. The level is the mean over a
                    // window that ends there, so the 1 kHz value stands for the window's centre.
                    let t = base + (self.n8 - 1) as f64 / W - self.delay_s;
                    lv.t0 = t - if lf {
                        0.010
                    } else {
                        self.corr.len() as f64 / 2.0 / W
                    };
                }
                lv.v.push(value);
            }
        }
        if cn > 0 {
            self.carrier = power_to_db(cpow / cn as f64);
        }
        lv.carrier_dbfs = self.carrier;
        lv.ticks = (power_to_db(self.tick_acc.0), power_to_db(self.tick_acc.1));
        io.items_out::<Levels>(0).push(Timed {
            time: lv.t0,
            value: lv,
        });
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match FrontSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            FrontSetting::Station => self.choice.pick = STATION_SETTING.read(id, v)?,
            FrontSetting::SixtyKhz => self.choice.sixty = SIXTY_KHZ.read(id, v)?,
            FrontSetting::AudioInput => self.audio_input = AUDIO_INPUT.read(id, v)?,
            FrontSetting::AudioToneHz => self.tone_hz = AUDIO_TONE_HZ.read(id, v)?,
            FrontSetting::ClockSlowPct => self.slow = CLOCK_SLOW_PCT.read(id, v)?,
            FrontSetting::Listen => {
                self.listen = LISTEN.read(id, v)?;
                return Ok(Applied::Live);
            }
        }
        Ok(Applied::Reformat)
    }
    fn values(&self) -> Values {
        values_of([
            (
                FrontSetting::Station,
                Value::Choice(self.choice.pick.index()),
            ),
            (
                FrontSetting::SixtyKhz,
                Value::Choice(self.choice.sixty.index()),
            ),
            (FrontSetting::AudioInput, Value::Bool(self.audio_input)),
            (FrontSetting::AudioToneHz, Value::Hz(self.tone_hz)),
            (FrontSetting::ClockSlowPct, Value::F64(self.slow)),
            (FrontSetting::Listen, Value::Bool(self.listen)),
        ])
    }
}

/// Reads one second from the normalised tenths of the second.
pub fn read_second(st: Station, u: &[f32; 10]) -> Sym {
    let hi = |k: usize| u[k] > 0.5;
    let mean = |a: usize, b: usize| u[a..=b].iter().sum::<f32>() / (b - a + 1) as f32;
    match st {
        Station::Dcf77 => {
            if hi(0) {
                Sym::None
            } else if !hi(1) {
                Sym::One
            } else {
                Sym::Zero
            }
        }
        Station::Msf => {
            if hi(0) {
                Sym::Unread
            } else if mean(3, 4) < 0.5 {
                Sym::Marker
            } else {
                let a = !hi(1) as u32;
                let b = !hi(2) as u32;
                match a + 2 * b {
                    0 => Sym::Zero,
                    1 => Sym::One,
                    2 => Sym::Two,
                    _ => Sym::Three,
                }
            }
        }
        Station::Wwvb => {
            if hi(0) {
                Sym::Unread
            } else if mean(5, 7) < 0.5 {
                Sym::Marker
            } else if mean(2, 4) < 0.5 {
                Sym::One
            } else {
                Sym::Zero
            }
        }
        Station::Jjy40 | Station::Jjy60 => {
            if !hi(0) {
                Sym::Unread
            } else if mean(2, 4) < 0.5 {
                Sym::Marker
            } else if mean(5, 7) < 0.5 {
                Sym::One
            } else {
                Sym::Zero
            }
        }
        Station::Bpc => {
            if hi(0) {
                Sym::None
            } else {
                let lows = (1..5).take_while(|&k| !hi(k)).count();
                match lows {
                    0 => Sym::Zero,
                    1 => Sym::One,
                    2 => Sym::Two,
                    _ => Sym::Three,
                }
            }
        }
        Station::Wwv => {
            if !hi(0) {
                Sym::None
            } else if mean(5, 7) > 0.5 {
                Sym::Marker
            } else if mean(2, 4) > 0.5 {
                Sym::One
            } else {
                Sym::Zero
            }
        }
        Station::Hla => {
            if !hi(0) {
                Sym::None
            } else if mean(5, 6) > 0.5 {
                Sym::Marker
            } else if mean(2, 3) > 0.5 {
                Sym::One
            } else {
                Sym::Zero
            }
        }
        Station::Bpm | Station::Bpmc | Station::Chu => {
            if !hi(0) {
                Sym::Unread
            } else if hi(2) {
                Sym::Marker
            } else if hi(1) {
                Sym::One
            } else {
                Sym::Zero
            }
        }
        Station::Rwm => {
            if hi(1) && hi(3) {
                Sym::Marker
            } else if hi(2) {
                Sym::Double
            } else {
                Sym::One
            }
        }
    }
}

/// One read second.
#[derive(Clone, Debug, PartialEq)]
pub struct SecondSym {
    /// Signal time of the second's edge.
    pub edge: f64,
    /// The symbol.
    pub sym: Sym,
}

impl Item for SecondSym {
    const KIND: &'static str = "time.symbol";
}

/// What the second finder measures.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SecondsStatus {
    /// The seconds are found.
    pub found: bool,
    /// Edge, ms after a whole second of signal time.
    pub tick_ms: f64,
    /// Tick jitter, ms.
    pub jitter_ms: f64,
    /// Keying depth (LF) or tick level over the background (HF), 0–1.
    pub depth: f64,
    /// SNR, dB.
    pub snr_db: f64,
    /// Carrier level, dBFS.
    pub carrier_dbfs: f64,
    /// 1000 and 1200 Hz tick levels.
    pub ticks: (f64, f64),
}

impl Item for SecondsStatus {
    const KIND: &'static str = "time.seconds";
}

/// The seconds finder and slicer (`time.seconds`: `time.level` to `time.symbol`). It takes the station from the level stream.
pub struct Seconds {
    station: Station,
    profile: Vec<f64>,
    filled: u64,
    hist: VecDeque<f32>,
    hist_t0: f64,
    n: i64,
    edge: Option<f64>,
    next_read: Option<f64>,
    status: SecondsStatus,
    singles: VecDeque<f64>,
    carrier: f64,
    ticks: (f64, f64),
}

impl Default for Seconds {
    fn default() -> Self {
        Seconds {
            station: Station::Dcf77,
            profile: vec![0.0; 1000],
            filled: 0,
            hist: VecDeque::new(),
            hist_t0: 0.0,
            n: 0,
            edge: None,
            next_read: None,
            status: SecondsStatus::default(),
            singles: VecDeque::new(),
            carrier: -150.0,
            ticks: (0.0, 0.0),
        }
    }
}

impl Seconds {
    fn restart(&mut self) {
        self.profile.iter_mut().for_each(|v| *v = 0.0);
        self.filled = 0;
        self.hist.clear();
        self.n = 0;
        self.edge = None;
        self.next_read = None;
        self.status = SecondsStatus::default();
    }

    fn pmean(&self, a: f64, b: f64) -> f64 {
        let (a, b) = (a.round() as i64, b.round() as i64);
        let mut s = 0.0;
        for j in a..b {
            s += self.profile[j.rem_euclid(1000) as usize];
        }
        s / (b - a).max(1) as f64
    }

    fn locate(&mut self) {
        let r = rising(self.station);
        let mut best = (f64::MIN, 0usize);
        let step = |s: &Self, j: i64| {
            let j = j as f64;
            let after = s.pmean(j + 3.0, j + 12.0);
            let before = s.pmean(j - 12.0, j - 3.0);
            if r {
                after - before
            } else {
                before - after
            }
        };
        if self.station == Station::Rwm {
            // RWM: the pulse that stays up longest (the second pulses are 100 ms, the 20 ms pulses shorter).
            for j in 0..1000i64 {
                let v = self.pmean(j as f64, j as f64 + 90.0)
                    - self.pmean(j as f64 - 30.0, j as f64 - 5.0);
                if v > best.0 {
                    best = (v, j as usize);
                }
            }
        } else {
            for j in 0..1000i64 {
                let v = step(self, j);
                if v > best.0 {
                    best = (v, j as usize);
                }
            }
        }
        let j = best.1 as i64;
        let (a, b, c) = (step(self, j - 1), best.0, step(self, j + 1));
        let den = a - 2.0 * b + c;
        let frac = if den.abs() > 1e-30 {
            (0.5 * (a - c) / den).clamp(-0.5, 0.5)
        } else {
            0.0
        };
        let (mx, mn) = self
            .profile
            .iter()
            .fold((f64::MIN, f64::MAX), |(x, n), &v| (x.max(v), n.min(v)));
        let range = mx - mn;
        let clear = range > 0.0 && best.0 > 0.3 * range;
        if !clear {
            self.status.found = false;
            self.edge = None;
            return;
        }
        let pos = (j as f64 + frac).rem_euclid(1000.0);
        self.edge = Some(pos);
        self.status.found = true;
        self.status.tick_ms = pos;
        let ((ha, hb), (la, lb)) = regions(self.station);
        let hi = self.pmean(pos + ha as f64, pos + hb as f64);
        let lo = self.pmean(pos + la as f64, pos + lb as f64);
        self.status.depth = if hi > 0.0 {
            ((hi - lo) / hi).clamp(0.0, 1.0)
        } else {
            0.0
        };
    }
}

impl Component for Seconds {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "time.seconds",
            vec![Port::typed::<Levels>("levels")],
            vec![
                Port::typed::<SecondSym>("symbols").rate_from(RateFrom::Fixed(0.0)),
                Port::typed::<SecondsStatus>("status").rate_from(RateFrom::Fixed(0.0)),
            ],
        )
    }
    fn values(&self) -> Values {
        sdrkit_graph::no_values()
    }
    fn configure(&mut self, _inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.restart();
        Ok(vec![
            Format::new(Kind::typed::<SecondSym>(), 0.0),
            Format::new(Kind::typed::<SecondsStatus>(), 0.0),
        ])
    }
    fn process(&mut self, io: &mut Io) {
        let levels = io.items::<Levels>(0);
        let out = io.items_out::<SecondSym>(0);
        for l in levels {
            let l = &l.value;
            let st = l.station.unwrap_or(Station::Dcf77);
            if st != self.station {
                self.station = st;
                self.restart();
            }
            self.carrier = l.carrier_dbfs;

            self.ticks = l.ticks;
            if self.hist.is_empty() {
                self.hist_t0 = l.t0;
                // Anchor the 1 kHz grid to signal time.
                self.n = (l.t0 * 1000.0).round() as i64;
            }
            for &v in &l.v {
                let j = self.n.rem_euclid(1000) as usize;
                let a = if self.filled < 10_000 {
                    1.0 / (self.filled / 1000 + 1) as f64
                } else {
                    0.1
                };
                self.profile[j] += a * (v as f64 - self.profile[j]);
                self.hist.push_back(v);
                self.n += 1;
                self.filled += 1;
                if self.hist.len() > 4000 {
                    self.hist.pop_front();
                    self.hist_t0 += 0.001;
                }
                if self.n.rem_euclid(1000) == 0 && self.filled >= 3000 {
                    self.locate();
                    if let Some(e) = self.edge {
                        let base = self.n.div_euclid(1000) as f64 - 3.0;
                        let first_edge = base + e / 1000.0;
                        // Keep the second count; follow the newest edge estimate.
                        let next = match self.next_read {
                            Some(t) => first_edge + (t - first_edge).round(),
                            None => first_edge,
                        };
                        self.next_read = Some(next);
                    } else {
                        self.next_read = None;
                    }
                }
                // Read every whole second whose 1000 ms are in the history.
                while let Some(tn) = self.next_read {
                    let end = self.hist_t0 + self.hist.len() as f64 / 1000.0;
                    if tn + 1.0 > end {
                        break;
                    }
                    let e = self.edge.unwrap_or(0.0);
                    if tn < self.hist_t0 {
                        self.next_read = Some(tn + 1.0);
                        continue;
                    }
                    let ((ha, hb), (la, lb)) = regions(self.station);
                    let hi = self.pmean(e + ha as f64, e + hb as f64);
                    let lo = self.pmean(e + la as f64, e + lb as f64);
                    let span = (hi - lo).abs().max(1e-12);
                    let start = ((tn - self.hist_t0) * 1000.0).round() as usize;
                    let mut u = [0f32; 10];
                    for (k, uk) in u.iter_mut().enumerate() {
                        let a = start + 100 * k + 10;
                        let b = start + 100 * k + 90;
                        let m: f64 = (a..b)
                            .map(|i| self.hist[i.min(self.hist.len() - 1)] as f64)
                            .sum::<f64>()
                            / (b - a) as f64;
                        *uk = ((m - lo) / span) as f32;
                    }
                    let sym = read_second(self.station, &u);
                    // SNR: span over the deviation in an always-high region of this second.
                    let (a, b) = (start + ha, (start + hb).min(self.hist.len()));
                    if b > a + 2 {
                        let n = (b - a) as f64;
                        let mean = (a..b).map(|i| self.hist[i] as f64).sum::<f64>() / n;
                        let sd = ((a..b)
                            .map(|i| (self.hist[i] as f64 - mean).powi(2))
                            .sum::<f64>()
                            / n)
                            .sqrt();
                        let snr = (20.0 * (span / sd.max(1e-12)).log10()).min(60.0);
                        self.status.snr_db += 0.2 * (snr - self.status.snr_db);
                    }
                    self.singles.push_back(tn.fract() * 1000.0);
                    if self.singles.len() > 60 {
                        self.singles.pop_front();
                    }
                    out.push(Timed {
                        time: tn,
                        value: SecondSym { edge: tn, sym },
                    });
                    self.next_read = Some(tn + 1.0);
                }
            }
        }
        if io.tick() {
            let mut st = self.status.clone();
            st.carrier_dbfs = self.carrier;
            st.ticks = self.ticks;
            if !self.singles.is_empty() {
                let m = st.tick_ms;
                let v = self.singles.iter().map(|s| {
                    let d = (s - m + 500.0).rem_euclid(1000.0) - 500.0;
                    d * d
                });
                st.jitter_ms = (v.sum::<f64>() / self.singles.len() as f64).sqrt();
            }
            io.items_out::<SecondsStatus>(1).push(Timed {
                time: 0.0,
                value: st,
            });
        }
    }
}

/// The path delay (s) from a station's site to the receiver at `rx`, the distance (m), and whether it is the ground wave. LF
/// stations take the ground wave. HF stations take hops off a layer 300 km high, at most 4000 km each.
pub fn path_delay(st: Station, rx: Position) -> (f64, f64, bool) {
    let site = st.info().site;
    let d = Position::new(site.0, site.1).distance_m(&rx);
    let c = sdrkit_util_geo::SPEED_OF_LIGHT_M_S;
    if st.info().lf {
        (d / c, d, true)
    } else {
        let n = (d / 4_000_000.0).ceil().max(1.0);
        let half = d / (2.0 * n);
        let path = n * 2.0 * (half * half + 300e3 * 300e3).sqrt();
        (path / c, d, false)
    }
}

/// One row of the minutes table.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MinuteRow {
    /// Signal time of the end of the minute, s.
    pub time_s: f64,
    /// The host time, when the samples are live.
    pub host_time: Option<f64>,
    /// Decoded.
    pub ok: bool,
    /// The time as the station sends it, for a decoded minute.
    pub text: String,
    /// Why the minute was refused.
    #[serde(default)]
    pub refusal: Option<Refusal>,
    /// The second where the decoder stopped.
    #[serde(default)]
    pub second: Option<usize>,
    /// The symbols.
    pub symbols: String,
}

impl Shift for MinuteRow {
    fn shift_times(&mut self, by_s: f64) {
        self.time_s += by_s;
    }
}

sdrkit_core::choice_enum! {
    /// How the path delay was found.
    pub enum PathKind {
        /// The ground wave of an LF station.
        GroundWave = ("ground_wave", "Ground wave"),
        /// Hops off a layer 300 km high.
        SkyWave = ("sky_wave", "Sky wave estimate"),
    }
}

sdrkit_core::choice_enum! {
    /// A day of the week.
    pub enum Day {
        /// Monday.
        Monday = ("monday", "Monday"),
        /// Tuesday.
        Tuesday = ("tuesday", "Tuesday"),
        /// Wednesday.
        Wednesday = ("wednesday", "Wednesday"),
        /// Thursday.
        Thursday = ("thursday", "Thursday"),
        /// Friday.
        Friday = ("friday", "Friday"),
        /// Saturday.
        Saturday = ("saturday", "Saturday"),
        /// Sunday.
        Sunday = ("sunday", "Sunday"),
    }
}

impl Day {
    /// The day of a weekday.
    pub fn of(w: sdrkit_util_geo::time::Weekday) -> Day {
        use sdrkit_util_geo::time::Weekday as W;
        match w {
            W::Monday => Day::Monday,
            W::Tuesday => Day::Tuesday,
            W::Wednesday => Day::Wednesday,
            W::Thursday => Day::Thursday,
            W::Friday => Day::Friday,
            W::Saturday => Day::Saturday,
            W::Sunday => Day::Sunday,
        }
    }
}

/// Minutes kept.
pub const MAX_MINUTES: usize = 200;

/// What `time` publishes.
pub fn declaration() -> ResultsDecl {
    crate::typed::shape("time").results_decl()
}

impl Shaped for MinuteRow {
    type Row = TimeMinuteRow<'static>;
    fn row(&self, _seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        out.put(&TimeMinuteRow {
            time_s: self.time_s,
            host_time: self.host_time.map(Time),
            ok: self.ok,
            text: (!self.text.is_empty()).then_some(self.text.as_str()),
            refusal: self.refusal,
            second: self.second.map(|s| s as i64),
            symbols: &self.symbols,
        });
    }
}

/// What the minute decoder keeps: the minutes log.
#[derive(Serialize, Deserialize)]
pub struct Store {
    /// The minutes, oldest first.
    pub minutes: LogRing<MinuteRow>,
}

impl Default for Store {
    fn default() -> Self {
        Store {
            minutes: LogRing::new(MAX_MINUTES),
        }
    }
}

impl Shift for Store {
    fn shift_times(&mut self, by_s: f64) {
        self.minutes.shift_times(by_s);
    }
}

impl Tables for Store {
    const FORMAT: StateFormat = StateFormat::new("science.time_minutes", 2);
    fn clear(&mut self) {
        self.minutes.clear();
    }
    fn adopt(&mut self, saved: Self) {
        self.minutes.adopt(saved.minutes);
    }
}

/// The minute assembler and time keeper (`time.minutes`).
pub struct Minutes {
    choice: StationChoice,
    station: Option<Station>,
    format: Option<Format>,
    rate: f64,
    t: f64,
    syms: VecDeque<SecondSym>,
    last: Option<(f64, TimeFrame)>,
    confirmed: bool,
    store: Store,
    decoded: u64,
    rejected: u64,
    clock: ClockMode,
    clock_choice: ClockChoice,
    clock_origin: f64,
    live: LiveClock,
    live_input: bool,
    host_clock: Clock,
    position: Option<Position>,
    offsets: VecDeque<(f64, f64)>,
    status: SecondsStatus,
    hist: [VecDeque<(f32, f32)>; 4],
    sec_acc: f64,
    year_setting: i64,
    year_hint: i64,
    dut1_ut1: f64,
    published: Published,
    series: [SeriesId; 4],
    shown: Status,
    events: Vec<Event>,
}

impl Default for Minutes {
    fn default() -> Self {
        let decl = std::sync::Arc::new(declaration());
        let series =
            TIME_SERIES.map(|d| decl.series_id(d.key).expect("the declaration holds the series"));
        Minutes {
            choice: StationChoice::default(),
            station: None,
            format: None,
            rate: 0.0,
            t: 0.0,
            syms: VecDeque::new(),
            last: None,
            confirmed: false,
            store: Store::default(),
            decoded: 0,
            rejected: 0,
            clock: ClockMode::of(CLOCK_MODE.default, CLOCK_ORIGIN.default),
            clock_choice: CLOCK_MODE.default,
            clock_origin: CLOCK_ORIGIN.default,
            live: LiveClock::default(),
            live_input: false,
            host_clock: Clock::fixed(0.0),
            position: None,
            offsets: VecDeque::new(),
            status: SecondsStatus::default(),
            hist: Default::default(),
            sec_acc: 0.0,
            year_setting: YEAR_HINT.default,
            year_hint: calendar::civil(0).year,
            dut1_ut1: 0.0,
            published: Published::from_decl(decl),
            series,
            shown: Status::default(),
            events: Vec::new(),
        }
    }
}

impl Minutes {
    fn st(&self) -> Station {
        self.station.unwrap_or(Station::Dcf77)
    }

    fn resolve(&mut self) {
        let tuned = self.format.as_ref().map_or(0.0, |f| f.tuned_hz);
        let st = self.choice.resolve(tuned);
        if st != self.station {
            self.station = st;
            self.syms.clear();
            self.last = None;
            self.confirmed = false;
        }
    }

    fn starts_minute(&self) -> bool {
        let n = self.syms.len();
        let s = |k: usize| self.syms[n - 1 - k].sym;
        match self.st() {
            Station::Dcf77 => n >= 2 && s(1) == Sym::None,
            Station::Msf | Station::Rwm => s(0) == Sym::Marker,
            Station::Wwvb | Station::Jjy40 | Station::Jjy60 | Station::Bpm => {
                n >= 2 && s(0) == Sym::Marker && s(1) == Sym::Marker
            }
            Station::Wwv | Station::Hla => n >= 2 && s(0) == Sym::None && s(1) == Sym::Marker,
            Station::Bpc => {
                n >= 61
                    && s(0) == Sym::None
                    && s(20) == Sym::None
                    && s(40) == Sym::None
                    && s(60) == Sym::None
                    && s(59) == Sym::Zero
            }
            Station::Chu | Station::Bpmc => false,
        }
    }

    fn symbol(&mut self, s: SecondSym) {
        self.syms.push_back(s);
        while self.syms.len() > 62 {
            self.syms.pop_front();
        }
        if !self.starts_minute() || self.syms.len() < 61 {
            return;
        }
        let n = self.syms.len();
        let mut m = [Sym::Unread; 60];
        for (k, x) in m.iter_mut().enumerate() {
            *x = self.syms[n - 61 + k].sym;
        }
        let t0 = self.syms[n - 61].edge;
        let text: String = m.iter().map(|s| s.ch()).collect();
        let host = wall(self.live_input, &self.host_clock);
        let st = self.st();
        match codes::decode(st, &m, self.year_hint) {
            Ok(f) => {
                self.decoded += 1;
                self.confirmed = self.last.as_ref().is_some_and(|(pt, pf)| {
                    let expect = pf.utc as f64 + (t0 - pt);
                    (expect - f.utc as f64).abs() < 1.5
                });
                let msg = f.text.clone();
                if f.year > 0 && self.year_setting == 0 {
                    self.year_hint = f.year;
                }
                self.events.push(TIME_MINUTE.event(
                    t0,
                    self.tuned_hz(),
                    &TimeMinuteEvent {
                        text: &msg,
                        family: Family::Other,
                        utc: Time(f.utc as f64),
                        station: st,
                    },
                ));
                self.dut1_ut1 = if f.ut1 { f.dut1.unwrap_or(0.0) } else { 0.0 };
                self.last = Some((t0, f));
                self.store.minutes.push(MinuteRow {
                    time_s: self.t,
                    host_time: host,
                    ok: true,
                    text: msg,
                    refusal: None,
                    second: None,
                    symbols: text,
                });
            }
            Err(why) => {
                self.rejected += 1;
                self.published
                    .results_mut()
                    .log(format!("{}: minute refused: {why}", st.info().name));
                self.store.minutes.push(MinuteRow {
                    time_s: self.t,
                    host_time: host,
                    ok: false,
                    text: String::new(),
                    refusal: Some(why.reason),
                    second: why.second,
                    symbols: text,
                });
            }
        }
    }

    fn tuned_hz(&self) -> f64 {
        self.format.as_ref().map_or(0.0, |f| f.tuned_hz)
    }

    fn wall_of(&self, t: f64) -> Option<f64> {
        match self.clock {
            ClockMode::None => None,
            ClockMode::Fixed(o) => Some(o + t),
            ClockMode::Live => self.live.origin().map(|o| o + t),
        }
    }

    fn publish(&mut self) {
        let Some(station) = self.station else {
            status(&mut self.shown, lock_state_of(0), None, None, None, |s| {
                s.push_str("Time signals: no station at this frequency")
            });
            self.published.results_mut().set_status(&self.shown);
            return;
        };
        let st = station.info();
        let state = if self
            .last
            .as_ref()
            .is_some_and(|(t0, _)| self.t - t0 < 180.0)
        {
            2
        } else if self.status.found {
            1
        } else {
            0
        };
        let path = self.position.map(|p| path_delay(station, p));
        let utc_now = self
            .last
            .as_ref()
            .map(|(t0, f)| f.utc as f64 + (self.t - t0));
        let wall = self.wall_of(self.t);
        let wwvh = (station == Station::Wwv).then_some(self.status.ticks.1 > self.status.ticks.0);
        let symbols: String = {
            let n = self.syms.len();
            let since = (0..n)
                .rev()
                .find(|&k| matches!(self.syms[k].sym, Sym::Marker | Sym::None));
            self.syms
                .iter()
                .skip(since.unwrap_or(0))
                .map(|s| s.sym.ch())
                .collect()
        };
        let no_position = self.position.is_none();
        status(
            &mut self.shown,
            lock_state_of(state),
            Some(self.status.snr_db as f32),
            Some(self.status.carrier_dbfs as f32),
            None,
            |s| {
                let _ = match (state, utc_now) {
                    (2, Some(u)) => {
                        write!(s, "{} {} UTC", st.name, calendar::iso(u.floor() as i64))
                    }
                    (1, _) => write!(s, "{}: seconds found, waiting for the minute", st.name),
                    _ => write!(s, "{}: searching", st.name),
                };
                if no_position && state == 2 {
                    s.push_str(" (no station position: no path delay)");
                }
            },
        );
        let p = &mut self.published;
        p.results_mut().set_status(&self.shown);
        let frame = self.last.as_ref().map(|x| &x.1);
        let delay = path.map_or(0.0, |x| x.0);
        let (clock_offset_s, clock_offset_avg_s) = match (utc_now, wall) {
            (Some(u), Some(w)) => {
                // Clock offset: host minus (received second plus path delay). In its UT1 minutes the BPM ticks come early by DUT1.
                let o = w - (u + delay - self.dut1_ut1);
                if self.offsets.back().is_none_or(|b| self.t - b.0 >= 1.0) {
                    self.offsets.push_back((self.t, o));
                    while self.offsets.len() > 60 {
                        self.offsets.pop_front();
                    }
                }
                let avg = self.offsets.iter().map(|x| x.1).sum::<f64>() / self.offsets.len() as f64;
                (Some(o), Some(avg))
            }
            _ => (None, None),
        };
        p.put(&TimeFields {
            station,
            wwvh,
            carrier_dbfs: self.status.carrier_dbfs,
            snr_db: self.status.snr_db,
            depth_pct: 100.0 * self.status.depth,
            tick_ms: self.status.tick_ms,
            jitter_ms: self.status.jitter_ms,
            minutes_ok: self.decoded as i64,
            minutes_bad: self.rejected as i64,
            confirmed: self.confirmed,
            distance_m: path.map(|x| x.1),
            delay_ms: path.map(|x| x.0 * 1000.0),
            path: path.map(|x| {
                if x.2 {
                    PathKind::GroundWave
                } else {
                    PathKind::SkyWave
                }
            }),
            symbols: &symbols,
            utc: utc_now.map(Time),
            received: frame.map(|f| f.text.as_str()),
            dut1_s: frame.and_then(|f| f.dut1),
            summer: frame.and_then(|f| f.summer),
            weekday: frame.and_then(|f| f.weekday).map(Day::of),
            clock_offset_s,
            clock_offset_avg_s,
        });
        log_rows(&self.store.minutes, &TIME_MINUTES, self.t, p);
        let r = p.results_mut();
        for (id, h) in self.series.iter().zip(&self.hist) {
            write_series(r, *id, true, h.iter().map(|&(a, b)| [a, b]));
        }
    }
}

sdrkit_core::settings! {
    /// The settings of [`Minutes`].
    pub enum MinutesSetting {
        /// The station to decode.
        Station = STATION_SETTING,
        /// The station of 60 kHz.
        SixtyKhz = SIXTY_KHZ,
        /// What the received time is compared with.
        ClockMode = CLOCK_MODE,
        /// The UTC of sample 0 for the fixed clock.
        ClockOrigin = CLOCK_ORIGIN,
        /// The year of the stations that send none.
        YearHint = YEAR_HINT,
    }
}

sdrkit_core::commands! {
    /// The commands of [`Minutes`].
    pub enum MinutesCommand {
        /// Forgets the minutes log.
        Clear = objcmd::CLEAR,
    }
}

impl Component for Minutes {
    fn any_ref(&self) -> Option<&dyn std::any::Any> {
        Some(self)
    }
    fn any_mut(&mut self) -> Option<&mut dyn std::any::Any> {
        Some(self)
    }
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "time.minutes",
            vec![
                Port::iq("in"),
                Port::typed::<SecondSym>("symbols"),
                Port::typed::<SecondsStatus>("status"),
            ],
            vec![Port::new("events", Kind::Messages).rate_from(RateFrom::Fixed(0.0))],
        )
        .with_settings(MinutesSetting::schema())
    }
    fn attach(&mut self, ctx: &RunContext) {
        self.position = ctx.station.position;
        self.live_input = ctx.live;
        self.host_clock = ctx.clock.clone();
        self.year_hint = if self.year_setting > 0 {
            self.year_setting
        } else {
            let now = match self.clock {
                ClockMode::Fixed(origin) => origin,
                ClockMode::None | ClockMode::Live => self.host_clock.now(),
            };
            calendar::civil(now as i64).year
        };
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.rate = inputs[0].rate;
        self.format = Some(inputs[0].clone());
        if !self.live_input && self.clock == ClockMode::Live {
            self.clock = ClockMode::None;
        }
        self.syms.clear();
        self.resolve();
        self.publish();
        Ok(vec![Format::new(Kind::Messages, 0.0)])
    }
    fn process(&mut self, io: &mut Io) {
        for t in io.input(0).tags() {
            if let TagKind::Retuned {
                centre_hz,
                tuned_hz,
            } = t.kind
            {
                if let Some(f) = self.format.as_mut() {
                    f.centre_hz = centre_hz;
                    f.tuned_hz = tuned_hz;
                }
                self.resolve();
            }
        }
        let first = io.input(0).first();
        let n = io.input_len(0);
        self.t = (first + n as u64) as f64 / self.rate.max(1.0);
        if let (ClockMode::Live, Some(w)) = (self.clock, wall(self.live_input, &self.host_clock)) {
            self.live.arrive(w, self.t);
        }
        for s in io.items::<SecondsStatus>(2) {
            self.status = s.value.clone();
        }
        for s in io.items::<SecondSym>(1) {
            self.symbol(s.value.clone());
        }
        self.sec_acc += n as f64 / self.rate.max(1.0);
        if self.sec_acc >= 1.0 {
            self.sec_acc -= 1.0;
            let t = self.t as f32;
            let off = self.offsets.back().map(|x| x.1).unwrap_or(0.0);
            let vals = [
                self.status.carrier_dbfs,
                self.status.snr_db,
                self.status.tick_ms,
                off,
            ];
            for (h, v) in self.hist.iter_mut().zip(vals) {
                h.push_back((t, v as f32));
                while h.len() > 120 {
                    h.pop_front();
                }
            }
        }
        if io.tick() {
            self.publish();
        }
        if !self.events.is_empty() {
            io.messages_out(0).append(&mut self.events);
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match MinutesSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            MinutesSetting::Station => {
                self.choice.pick = STATION_SETTING.read(id, v)?;
                self.resolve();
            }
            MinutesSetting::SixtyKhz => {
                self.choice.sixty = SIXTY_KHZ.read(id, v)?;
                self.resolve();
            }
            MinutesSetting::ClockMode => {
                self.clock_choice = CLOCK_MODE.read(id, v)?;
                self.clock = ClockMode::of(self.clock_choice, self.clock_origin);
            }
            MinutesSetting::ClockOrigin => {
                self.clock_origin = CLOCK_ORIGIN.read(id, v)?;
                self.clock = ClockMode::of(self.clock_choice, self.clock_origin);
            }
            MinutesSetting::YearHint => {
                self.year_setting = YEAR_HINT.read(id, v)?;
                if self.year_setting > 0 {
                    self.year_hint = self.year_setting;
                }
            }
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([
            (
                MinutesSetting::Station,
                Value::Choice(self.choice.pick.index()),
            ),
            (
                MinutesSetting::SixtyKhz,
                Value::Choice(self.choice.sixty.index()),
            ),
            (
                MinutesSetting::ClockMode,
                Value::Choice(self.clock_choice.index()),
            ),
            (MinutesSetting::ClockOrigin, Value::F64(self.clock_origin)),
            (MinutesSetting::YearHint, Value::I64(self.year_setting)),
        ])
    }
    fn commands(&self) -> &Commands {
        MinutesCommand::commands()
    }
    fn invoke(&mut self, id: CommandId, _args: &Values) -> Result<Option<Record>, CommandError> {
        match MinutesCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            MinutesCommand::Clear => self.store.clear(),
        }
        self.publish();
        Ok(None)
    }
    fn results(&self) -> Option<&Results> {
        Some(self.published.results())
    }
    fn save_state(&self) -> Option<StateBlob> {
        self.store.save_state(self.t).ok()
    }
    fn restore_state(&mut self, state: &StateBlob) -> Result<(), StateError> {
        self.store.restore_state(state, self.t)?;
        self.publish();
        Ok(())
    }
}

/// The lowest input rate.
pub const MIN_RATE: f64 = 8e3;

/// The receiver: the front end, the seconds and the minutes. With `station` set to `auto` the station follows the dial.
///
/// # Errors
///
/// The errors of [`GraphBuilder::build`] and [`StackGraphBuilder::build`](sdrkit_graph::StackGraphBuilder::build).
pub fn receiver(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    let graph = GraphBuilder::new()
        .add("front", Front::default())
        .add("seconds", Seconds::default())
        .add("minutes", Minutes::default())
        .input("front")
        .input("minutes.in")
        .connect("front.levels", "seconds")
        .connect("seconds.symbols", "minutes.symbols")
        .connect("seconds.status", "minutes.status")
        .output("front.audio")
        .output("minutes.events")
        .clock(REPORT_S)
        .status_from("minutes")
        .declare(&declaration())
        .catalog(&Catalog::of(cfg)?)
        .build()?;
    StackGraph::of(crate::typed::shape("time"), graph)
        .min_rate(
            MIN_RATE,
            "Time signals need a sample rate of at least 8 ksps",
        )
        .route(STATION_SETTING.key(), &["front.station", "minutes.station"])?
        .route(SIXTY_KHZ.key(), &["front.sixty_khz", "minutes.sixty_khz"])?
        .route(AUDIO_INPUT.key(), &["front.audio_input"])?
        .route(AUDIO_TONE_HZ.key(), &["front.audio_tone_hz"])?
        .route(CLOCK_SLOW_PCT.key(), &["front.clock_slow_pct"])?
        .route(LISTEN.key(), &["front.listen"])?
        .route(CLOCK_MODE.key(), &["minutes.clock_mode"])?
        .route(CLOCK_ORIGIN.key(), &["minutes.clock_origin"])?
        .route(YEAR_HINT.key(), &["minutes.year_hint"])?
        .command(
            objcmd::CLEAR.key,
            sdrkit_graph::CommandTarget::of("minutes", "clear"),
        )
        .with_settings(&cfg.settings)
        .build()
}

/// The receiver as the engine takes it.
pub fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    crate::stack_receiver(receiver(cfg))
}

/// Generator options.
#[derive(Clone, Debug)]
pub struct TimeGenOptions {
    /// The station.
    pub station: Station,
    /// UTC of sample 0.
    pub start_utc: f64,
    /// What the time code sends.
    pub enc: EncodeOptions,
}

impl TimeGenOptions {
    /// Defaults for a station, starting at [`crate::family::TEST_START_UTC`].
    pub fn new(station: Station) -> TimeGenOptions {
        TimeGenOptions {
            station,
            start_utc: crate::family::TEST_START_UTC,
            enc: EncodeOptions::default(),
        }
    }
}

/// The time-signal generator: the station's carrier at 0 Hz, keyed (LF) or amplitude-modulated (HF) with its time code. The
/// carrier has amplitude 1 while it is up.
pub struct TimeGenerator {
    rate: f64,
    o: TimeGenOptions,
    n: u64,
    cache: Option<(i64, [Sym; 60])>,
}

impl TimeGenerator {
    /// At `rate`.
    pub fn new(rate: f64, o: TimeGenOptions) -> TimeGenerator {
        TimeGenerator {
            rate,
            o,
            n: 0,
            cache: None,
        }
    }

    /// From the stack's settings. With `station` set to `auto` it sends DCF77.
    pub fn from_config(cfg: &GenConfig) -> TimeGenerator {
        let s = &cfg.settings;
        let st = s.get(&STATION_SETTING).station().unwrap_or(Station::Dcf77);
        let mut o = TimeGenOptions::new(st);
        let start = s.get(&GEN_START_UTC);
        if start > 0.0 {
            o.start_utc = start;
        }
        TimeGenerator::new(cfg.rate_hz, o)
    }

    fn minute(&mut self, m0: i64) -> [Sym; 60] {
        if let Some((t, m)) = self.cache {
            if t == m0 {
                return m;
            }
        }
        let m = codes::encode(self.o.station, m0, &self.o.enc);
        self.cache = Some((m0, m));
        m
    }

    /// (carrier amplitude factor, AM audio) at a UTC instant.
    fn keying(&mut self, utc: f64) -> (f64, f64) {
        let st = self.o.station;
        let sec = utc.floor() as i64;
        let ms = (utc - sec as f64) * 1000.0;
        let m0 = sec - sec.rem_euclid(60);
        let s = (sec - m0) as usize;
        let sym = self.minute(m0)[s];
        let low_for = |a: f64, b: f64| ms >= a && ms < b;
        match st {
            Station::Dcf77 => {
                let d = match sym {
                    Sym::None => 0.0,
                    Sym::One => 200.0,
                    _ => 100.0,
                };
                (if ms < d { 0.15 } else { 1.0 }, 0.0)
            }
            Station::Msf => {
                let v = sym.val().unwrap_or(0);
                let off = if sym == Sym::Marker {
                    ms < 500.0
                } else {
                    ms < 100.0
                        || (v & 1 == 1 && low_for(100.0, 200.0))
                        || (v & 2 == 2 && low_for(200.0, 300.0))
                };
                (if off { 0.0 } else { 1.0 }, 0.0)
            }
            Station::Wwvb => {
                let d = match sym {
                    Sym::Marker => 800.0,
                    Sym::One => 500.0,
                    _ => 200.0,
                };
                (
                    if ms < d {
                        10f64.powf(-17.0 / 20.0)
                    } else {
                        1.0
                    },
                    0.0,
                )
            }
            Station::Jjy40 | Station::Jjy60 => {
                let d = match sym {
                    Sym::Marker => 200.0,
                    Sym::One => 500.0,
                    _ => 800.0,
                };
                (if ms < d { 1.0 } else { 0.1 }, 0.0)
            }
            Station::Bpc => {
                let d = match sym {
                    Sym::None => 0.0,
                    s => 100.0 * (s.val().unwrap_or(0) + 1) as f64,
                };
                (if ms < d { 0.1 } else { 1.0 }, 0.0)
            }
            Station::Rwm => {
                let on = match sym {
                    Sym::Marker => ms < 500.0,
                    Sym::Double => ms < 100.0 || low_for(200.0, 300.0),
                    _ => ms < 100.0,
                };
                (if on { 1.0 } else { 0.0 }, 0.0)
            }
            Station::Wwv | Station::Hla => {
                let (t1, t2, t3) = if st == Station::Hla {
                    (170.0, 470.0, 770.0)
                } else {
                    (200.0, 500.0, 800.0)
                };
                let d = match sym {
                    Sym::Marker => t3,
                    Sym::One => t2,
                    Sym::None => 0.0,
                    _ => t1,
                };
                let sub = if ms < d {
                    0.5 * (TAU * 100.0 * utc).sin()
                } else {
                    0.0
                };
                let tick = if st == Station::Wwv && ms < 5.0 && s != 29 && s != 59 {
                    0.6 * (TAU * 1000.0 * utc).sin()
                } else if st == Station::Hla && (968.0..974.0).contains(&ms) {
                    0.6 * (TAU * 1800.0 * utc).sin()
                } else {
                    0.0
                };
                (1.0, sub + tick)
            }
            Station::Bpm => {
                let d = match sym {
                    Sym::Marker => 300.0,
                    Sym::One => 200.0,
                    _ => 100.0,
                };
                (
                    1.0,
                    if ms < d {
                        0.5 * (TAU * 1000.0 * utc).sin()
                    } else {
                        0.0
                    },
                )
            }
            Station::Chu | Station::Bpmc => {
                let tick = if ms < 10.0 {
                    0.5 * (TAU * 1000.0 * utc).sin()
                } else {
                    0.0
                };
                (1.0, tick)
            }
        }
    }
}

impl Generator for TimeGenerator {
    fn rate_hz(&self) -> Hz {
        self.rate
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        for o in out.iter_mut() {
            let utc = self.o.start_utc + self.n as f64 / self.rate;
            // In its UT1 minutes BPM sends its ticks on UT1 = UTC + DUT1.
            let tick_utc = if self.o.station == Station::Bpm && self.o.enc.ut1 {
                utc + self.o.enc.dut1 as f64 / 10.0
            } else {
                utc
            };
            let (a, audio) = self.keying(tick_utc);
            *o = Complex32::new((a * (1.0 + 0.6 * audio)) as f32, 0.0);
            self.n += 1;
        }
        out.len()
    }
    fn signal_info(&self) -> Option<SignalInfo> {
        // The SNR of a time signal is its carrier to noise in 1 kHz.
        Some(SignalInfo::new(1e3))
    }
    fn station_offset_hz(&self, _tune_offset_hz: Hz) -> Hz {
        0.0
    }
}

/// The generator. The SAME, time-signal and e-Czas generators must never be routed to a transmitting radio.
pub fn make_generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(Box::new(TimeGenerator::from_config(cfg)))
}

/// The verify case: 200 s of DCF77 from a fixed start.
pub static VERIFY: VerifyHooks = VerifyHooks {
    cases: verify_cases,
    vector_settings: None,
    check_vector: None,
};

fn verify_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let mut c = VerifyCase::new("test signal, DCF77", o, 0.0);
    c.seconds = o.seconds.max(200.0);
    c.expect_audio = Some(false);
    // Five seconds before a minute, so the first whole minute ends after 65 s.
    c.settings
        .push(CaseSetting::new(&GEN_START_UTC, 1_791_813_595.0));
    c.check = Some(|r| {
        let n = crate::family::field(r, TimeFields::MINUTES_OK.key);
        vec![CheckLine::new(
            TimeFields::MINUTES_OK.key,
            n >= 1.0,
            format!("{n}, at least 1 expected"),
        )]
    });
    vec![c]
}
