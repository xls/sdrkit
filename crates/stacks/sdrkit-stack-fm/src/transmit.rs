//! The sender of the FM stack (`fm.transmit`): a continuous FM broadcast signal from the host's audio, in stereo with the
//! pilot and RDS, made by the multiplex of [`crate::mpx`] that also makes the test signal.
//!
//! Input `in` is the session's samples: their rate, pace and dial. Input `audio` is stereo audio of any rate, left
//! then right; the graph puts mono audio in both channels. Output `out` is complex baseband at the session's rate for
//! `tx.sink`, centred on the dial; its magnitude is 0.9 of full scale. Output `keyed` mutes the receive audio of the stack
//! from `start` to a short hold after the end mark. Each pass writes as many samples as the pass brings in.
//!
//! `start` builds the multiplex from the settings and sends until `stop`. `stop` ends at once: the carrier falls in a 5 ms
//! raised cosine, as it rose, and the last sample carries the end mark. The transmitter going off and a new format end the
//! transmission too. On a live radio whose host times bursts (`HostStreams::transmit_lead`), the first sample states its
//! time on air (`TagKind::BurstStart`). The programme content (PI, PS, RT, PTY, the flags, AF and the clock) changes
//! while the sender runs, at the next cycle of its RDS groups; the levels, stereo, pre-emphasis and the limiter apply at the
//! next start. The multiplex never passes `tx_deviation`: the audio part gets what the pilot and RDS leave.
//! Standard: ITU-R BS.450, ITU-R BS.412 (multiplex power), IEC 62106.

use crate::mpx::{Multiplex, MpxParams, Programme, AUDIO_RATE};
use crate::rds::encode::RdsContent;
use crate::settings::*;
use crate::typed::{FmTxFields, START, STOP};
use sdrkit_algo_dsp::sample::Stereo;
use sdrkit_components::tx::{end_mark, BurstOut, BurstTiming, KeyedOut, MicInput};
use sdrkit_core::interface::{Fields, Hertz};
use sdrkit_core::{
    Choice, Clock, CommandError, CommandId, CommandSet, Commands, Complex32, ConfigError,
    Published, Record, Results, SettingError, SettingId, SettingSet, StageError, Value, Values,
};
use sdrkit_graph::{Applied, Component, ComponentInfo, Format, Io, Port, RunContext};
use std::collections::VecDeque;

/// The name of the component.
pub const ID: &str = "fm.transmit";
/// The magnitude of the carrier.
pub const PEAK: f32 = 0.9;
/// The rise and the fall of the carrier (raised cosine), seconds.
pub const RISE_S: f64 = 0.005;
/// The host audio kept while it waits, seconds; older audio is dropped.
const AUDIO_KEEP_S: f64 = 0.25;
/// Silence put before the audio at the start, seconds: a reserve that covers the samples made ahead of the pass and the
/// unevenness of the host, so that the multiplex never runs short of audio.
const AUDIO_RESERVE_S: f64 = 0.02;
/// The signal made in one step, seconds.
const CHUNK_S: f64 = 0.004;
/// The time the multiplex power is averaged over, seconds (ITU-R BS.412).
const POWER_WINDOW_S: usize = 60;
/// The deviation of the sine whose power is the limit of the multiplex power, Hz (ITU-R BS.412).
const POWER_REF_HZ: f64 = 19_000.0;
/// The most that the pilot and RDS may take of the deviation.
const MOST_PILOT_RDS: f64 = 0.5;

sdrkit_core::settings! {
    /// The settings of the sender.
    pub enum TxSetting {
        /// The programme identification.
        Pi = TX_PI,
        /// The station name.
        Ps = TX_PS,
        /// The radio text.
        Rt = TX_RT,
        /// The programme type.
        Pty = TX_PTY,
        /// The traffic programme flag.
        Tp = TX_TP,
        /// The traffic announcement flag.
        Ta = TX_TA,
        /// The music flag.
        Music = TX_MUSIC,
        /// The alternative frequencies.
        Af = TX_AF,
        /// The clock group.
        Ct = TX_CT,
        /// The local time offset.
        CtOffset = TX_CT_OFFSET,
        /// Stereo.
        Stereo = TX_STEREO,
        /// The pre-emphasis.
        Preemphasis = TX_PREEMPHASIS,
        /// The peak deviation.
        Deviation = TX_DEVIATION,
        /// The pilot level.
        Pilot = TX_PILOT,
        /// The RDS level.
        RdsLevel = TX_RDS_LEVEL,
        /// The audio limiter.
        Limiter = TX_LIMITER,
    }
}

sdrkit_core::commands! {
    /// The commands of the sender.
    pub enum TxCommand {
        /// Starts the transmission.
        Start = START,
        /// Ends the transmission.
        Stop = STOP,
    }
}

/// The keys of the commands of the sender, which the stack routes to it.
pub const COMMANDS: [&str; 2] = [START.key, STOP.key];

/// Why `start` was refused.
#[derive(Clone, Debug, PartialEq)]
pub enum FmTxError {
    /// The setting `tx_pi` is empty.
    NoPi,
    /// The stack has no input yet, so it has no rate and no frequency.
    NoInput,
    /// The pilot and RDS take more than half of the deviation: their share.
    NoRoom(f64),
    /// The transmission is ending; start again after its end mark.
    Ending,
}

impl FmTxError {
    /// The key of the setting the error is about, if any.
    pub fn setting(&self) -> Option<&'static str> {
        match self {
            FmTxError::NoPi => Some(TX_PI.key()),
            FmTxError::NoRoom(_) => Some(TX_RDS_LEVEL.key()),
            _ => None,
        }
    }
}

impl std::fmt::Display for FmTxError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FmTxError::NoPi => write!(
                f,
                "the setting {} (PI code) is empty: enter the programme identification of the station",
                TX_PI.key()
            ),
            FmTxError::NoInput => f.write_str("the stack has no input yet"),
            FmTxError::NoRoom(share) => write!(
                f,
                "the pilot and the setting {} take {:.0} % of the deviation; at most {:.0} % is allowed",
                TX_RDS_LEVEL.key(),
                share * 100.0,
                MOST_PILOT_RDS * 100.0
            ),
            FmTxError::Ending => f.write_str("the transmission is ending"),
        }
    }
}

impl std::error::Error for FmTxError {}

/// A refusal of the sender as a command error.
fn failed(e: FmTxError) -> CommandError {
    CommandError::Failed {
        source: StageError::new(e),
    }
}

/// The values of the settings.
#[derive(Clone, Debug, PartialEq)]
struct TxConfig {
    pi: Option<u16>,
    ps: String,
    rt: String,
    rt_ab: bool,
    pty: u8,
    tp: bool,
    ta: bool,
    music: bool,
    af_text: String,
    af: Vec<u8>,
    ct: bool,
    ct_offset: i8,
    stereo: bool,
    preemphasis: Preemphasis,
    deviation: f64,
    pilot_pct: f64,
    rds_khz: f64,
    limiter: bool,
}

impl Default for TxConfig {
    fn default() -> Self {
        TxConfig {
            pi: None,
            ps: String::new(),
            rt: String::new(),
            rt_ab: false,
            pty: 0,
            tp: TX_TP.default,
            ta: TX_TA.default,
            music: TX_MUSIC.default,
            af_text: String::new(),
            af: Vec::new(),
            ct: TX_CT.default,
            ct_offset: 0,
            stereo: TX_STEREO.default,
            preemphasis: TX_PREEMPHASIS.default,
            deviation: TX_DEVIATION.default,
            pilot_pct: TX_PILOT.default,
            rds_khz: TX_RDS_LEVEL.default,
            limiter: TX_LIMITER.default,
        }
    }
}

impl TxConfig {
    /// The RDS content with programme identification `pi`.
    fn content(&self, pi: u16) -> RdsContent {
        RdsContent {
            pi,
            ps: self.ps.clone(),
            rt: self.rt.clone(),
            rt_ab: self.rt_ab,
            pty: self.pty,
            tp: self.tp,
            ta: self.ta,
            music: self.music,
            stereo: self.stereo,
            af_codes: self.af.clone(),
            extra_groups: Vec::new(),
            clock_offset: self.ct.then_some(self.ct_offset),
        }
    }

    /// The levels of the multiplex at `rate`: the audio part is what the pilot and RDS leave of the deviation.
    fn mpx_params(&self, rate: f64) -> Result<MpxParams, FmTxError> {
        let pilot = if self.stereo {
            self.pilot_pct / 100.0
        } else {
            0.0
        };
        let rds = self.rds_khz * 1e3 / self.deviation;
        if pilot + rds > MOST_PILOT_RDS {
            return Err(FmTxError::NoRoom(pilot + rds));
        }
        Ok(MpxParams {
            rate_hz: rate,
            deviation_hz: self.deviation,
            stereo: self.stereo,
            pilot,
            rds,
            audio: 1.0 - pilot - rds,
            offset: 0.0,
            sca: false,
        })
    }
}

/// The programme identification of `text`: one to four hexadecimal digits, not 0. `None` for an empty text.
fn parse_pi(id: SettingId, text: &str) -> Result<Option<u16>, SettingError> {
    let t = text.trim();
    if t.is_empty() {
        return Ok(None);
    }
    match u16::from_str_radix(t, 16) {
        Ok(pi) if pi != 0 && t.len() <= 4 => Ok(Some(pi)),
        _ => Err(SettingError::Malformed { id, at: Some(0) }),
    }
}

/// The AF codes of a list of frequencies in MHz: 87.6 to 107.9 MHz on the 0.1 MHz raster, at most 25.
fn parse_af(id: SettingId, text: &str) -> Result<Vec<u8>, SettingError> {
    let mut out = Vec::new();
    let mut at = 0;
    for part in text.split(|c: char| c.is_whitespace() || c == ',') {
        let start = at;
        at += part.len() + 1;
        if part.is_empty() {
            continue;
        }
        let bad = SettingError::Malformed {
            id,
            at: Some(start),
        };
        let mhz: f64 = part.parse().map_err(|_| bad.clone())?;
        let code = ((mhz - 87.5) * 10.0).round();
        if !(1.0..=204.0).contains(&code) || (87.5 + code / 10.0 - mhz).abs() > 0.005 || out.len() == 25 {
            return Err(bad);
        }
        out.push(code as u8);
    }
    Ok(out)
}

/// The text of `v` if it has at most `most` characters.
fn bounded(id: SettingId, v: &str, most: usize) -> Result<String, SettingError> {
    match v.char_indices().nth(most) {
        Some((at, _)) => Err(SettingError::Malformed { id, at: Some(at) }),
        None => Ok(v.to_owned()),
    }
}

/// The audio of the host as it waits for the multiplex: left and right, each at [`AUDIO_RATE`].
struct HostAudio {
    left: MicInput,
    right: MicInput,
    l: Vec<f32>,
    r: Vec<f32>,
    frames: Vec<Stereo>,
}

impl HostAudio {
    fn new() -> Self {
        HostAudio {
            left: MicInput::new(AUDIO_RATE, AUDIO_KEEP_S),
            right: MicInput::new(AUDIO_RATE, AUDIO_KEEP_S),
            l: Vec::new(),
            r: Vec::new(),
            frames: Vec::new(),
        }
    }

    fn configure(&mut self, f: &Format) {
        self.left.configure(f);
        self.right.configure(f);
    }

    fn restart(&mut self) {
        self.left.restart();
        self.right.restart();
    }

    /// Takes the host of `ctx`. The left input tells the host of each pass that keeps audio; one of the two is enough.
    fn attach(&mut self, ctx: &RunContext) {
        self.left.attach(ctx);
    }

    /// Takes the audio of input `i`: the first two channels as left and right, one channel as both.
    fn take(&mut self, io: &Io, i: usize) {
        if !io.is_connected(i) {
            return;
        }
        let a = io.audio(i);
        let ch = usize::from(a.channels.max(1));
        self.l.clear();
        self.r.clear();
        for f in a.samples.chunks_exact(ch) {
            self.l.push(f[0]);
            self.r.push(f[ch.min(2) - 1]);
        }
        self.left.push(&self.l, 1);
        self.right.push(&self.r, 1);
    }

    /// The frames that wait.
    fn waiting(&self) -> usize {
        self.left.waiting().min(self.right.waiting())
    }

    /// The next `n` frames, silence where none wait.
    fn read(&mut self, n: usize) -> &[Stereo] {
        self.l.resize(n, 0.0);
        self.r.resize(n, 0.0);
        self.left.read(&mut self.l);
        self.right.read(&mut self.r);
        self.frames.clear();
        self.frames
            .extend(self.l.iter().zip(&self.r).map(|(&l, &r)| Stereo::new(l, r)));
        &self.frames
    }
}

/// One transmission: the multiplex, the envelope of the carrier and the paced output.
struct Burst {
    mpx: Multiplex,
    out: BurstOut,
    rate: f64,
    rise: usize,
    chunk: usize,
    made: u64,
    fall_from: Option<u64>,
    owed: f64,
    base: Vec<Complex32>,
    peak: f32,
    second: (f64, u64),
    seconds: VecDeque<(f64, u64)>,
}

impl Burst {
    /// The magnitude of the carrier at sample `i` with a rise and fall of `rise` samples and the fall from `fall_from`:
    /// raised cosines, and [`PEAK`] between them.
    fn envelope(rise: usize, fall_from: Option<u64>, i: u64) -> f32 {
        let rise = rise as f64;
        let mut g = if (i as f64) < rise {
            0.5 - 0.5 * (std::f64::consts::PI * i as f64 / rise).cos()
        } else {
            1.0
        };
        if let Some(f) = fall_from {
            let j = i.saturating_sub(f) as f64;
            g *= if j < rise {
                0.5 + 0.5 * (std::f64::consts::PI * j / rise).cos()
            } else {
                0.0
            };
        }
        (g as f32) * PEAK
    }

    /// Makes samples until at least `room` output samples wait, or until the fall is made with the end of the output.
    fn fill(&mut self, room: usize, audio: &mut HostAudio) {
        while self.out.wants(room) {
            let mut n = self.chunk;
            if let Some(f) = self.fall_from {
                n = n.min((f + self.rise as u64).saturating_sub(self.made) as usize);
            }
            if n > 0 {
                // The audio owed is read as it comes; the reserve covers the rest, and audio owed beyond the reserve is
                // given up, so the delay stays bounded when the host is late.
                self.owed = (self.owed + n as f64 * AUDIO_RATE / self.rate)
                    .min(AUDIO_RESERVE_S * AUDIO_RATE);
                let k = (self.owed.floor() as usize).min(audio.waiting());
                self.owed -= k as f64;
                let frames = audio.read(k);
                self.mpx.push_audio(frames);
                self.base.resize(n, Complex32::default());
                self.mpx.generate(&mut self.base);
                let (rise, fall, made) = (self.rise, self.fall_from, self.made);
                for (i, z) in self.base.iter_mut().enumerate() {
                    *z *= Self::envelope(rise, fall, made + i as u64);
                }
                self.made += n as u64;
                self.out.push(&self.base);
                self.measure();
            }
            if self.fall_from.is_some_and(|f| self.made >= f + self.rise as u64) {
                self.out.finish(&mut self.base);
            }
        }
    }

    /// Takes the peak and the power of the multiplex made.
    fn measure(&mut self) {
        let (peak, energy, count) = self.mpx.take_level();
        self.peak = self.peak.max(peak);
        self.second.0 += energy;
        self.second.1 += count;
        if self.second.1 as f64 >= self.rate {
            if self.seconds.len() == POWER_WINDOW_S {
                self.seconds.pop_front();
            }
            self.seconds.push_back(std::mem::take(&mut self.second));
        }
    }

    /// The multiplex power over the last 60 s (or the time sent, at least a second) against the limit of ITU-R BS.412.
    fn power_db(&self) -> Option<f64> {
        let (e, n) = self
            .seconds
            .iter()
            .fold((0.0, 0u64), |a, s| (a.0 + s.0, a.1 + s.1));
        let deviation = self.mpx.params().deviation_hz;
        (n > 0).then(|| {
            let mean = e / n as f64 * deviation * deviation;
            10.0 * (mean / (POWER_REF_HZ * POWER_REF_HZ / 2.0)).log10()
        })
    }
}

/// The sender (`fm.transmit`). See the module.
pub struct FmTransmit {
    cfg: TxConfig,
    rate: f64,
    dial: f64,
    transmit: bool,
    audio: HostAudio,
    burst: Option<Burst>,
    end_owed: bool,
    keyed: KeyedOut,
    timing: BurstTiming,
    wall: Clock,
    pass_s: f64,
    last_groups: u64,
    results: Published,
}

impl Default for FmTransmit {
    fn default() -> Self {
        let mut t = FmTransmit {
            cfg: TxConfig::default(),
            rate: 0.0,
            dial: 0.0,
            transmit: false,
            audio: HostAudio::new(),
            burst: None,
            end_owed: false,
            keyed: KeyedOut::default(),
            timing: BurstTiming::default(),
            wall: Clock::fixed(0.0),
            pass_s: 0.0,
            last_groups: 0,
            results: Published::new(FmTxFields::SHAPE.fields),
        };
        t.publish();
        t
    }
}

impl FmTransmit {
    /// Whether a transmission is under way.
    pub fn transmitting(&self) -> bool {
        self.burst.is_some()
    }

    /// Whether the receive audio is muted: a transmission, its end mark or the hold after it.
    pub fn keyed(&self) -> bool {
        self.keyed.keyed(self.burst.is_some() || self.end_owed)
    }

    /// Drops the transmission on air; its end mark goes out on the next pass.
    fn cut(&mut self) {
        if let Some(b) = self.burst.take() {
            self.last_groups = b.mpx.groups();
            self.end_owed = true;
        }
    }

    /// Starts the transmission; one under way goes on.
    ///
    /// # Errors
    ///
    /// [`CommandError::TransmitOff`], or [`FmTxError`] when the setting `tx_pi` is empty, the stack has no input, the pilot
    /// and RDS leave too little of the deviation, or the transmission is ending.
    pub fn start(&mut self) -> Result<(), CommandError> {
        if !self.transmit {
            return Err(CommandError::TransmitOff);
        }
        if let Some(b) = &self.burst {
            return match b.fall_from {
                Some(_) => Err(failed(FmTxError::Ending)),
                None => Ok(()),
            };
        }
        let pi = self.cfg.pi.ok_or_else(|| failed(FmTxError::NoPi))?;
        if self.rate <= 0.0 {
            return Err(failed(FmTxError::NoInput));
        }
        let p = self.cfg.mpx_params(self.rate).map_err(failed)?;
        let programme = Programme::Live {
            preemphasis_s: self.cfg.preemphasis.tau_s(),
            limiter: self.cfg.limiter,
        };
        let mut mpx = Multiplex::new(p, &programme, self.cfg.content(pi))
            .map_err(|_| failed(FmTxError::NoInput))?;
        mpx.set_clock(
            self.timing
                .start_time(self.pass_s)
                .unwrap_or_else(|| self.wall.now()),
        );
        mpx.push_audio(&vec![Stereo::default(); (AUDIO_RESERVE_S * AUDIO_RATE) as usize]);
        let out = BurstOut::new(self.rate, self.rate, PEAK).map_err(|_| failed(FmTxError::NoInput))?;
        self.audio.restart();
        self.last_groups = 0;
        self.burst = Some(Burst {
            mpx,
            out,
            rate: self.rate,
            rise: ((RISE_S * self.rate).round() as usize).max(1),
            chunk: ((CHUNK_S * self.rate).round() as usize).max(1),
            made: 0,
            fall_from: None,
            owed: 0.0,
            base: Vec::new(),
            peak: 0.0,
            second: (0.0, 0),
            seconds: VecDeque::with_capacity(POWER_WINDOW_S),
        });
        Ok(())
    }

    /// Ends the transmission at once: the carrier falls after the samples made.
    pub fn stop(&mut self) {
        if let Some(b) = &mut self.burst {
            b.fall_from.get_or_insert(b.made);
        }
    }

    /// Gives a running transmission the programme content of the settings.
    fn content_changed(&mut self) {
        if let (Some(b), Some(pi)) = (&mut self.burst, self.cfg.pi) {
            // Stereo applies at the next start: the decoder identification follows the signal on air.
            let c = RdsContent {
                stereo: b.mpx.params().stereo,
                ..self.cfg.content(pi)
            };
            b.mpx.set_content(c);
        }
    }

    /// Writes the result fields.
    fn publish(&mut self) {
        let b = self.burst.as_mut();
        let on = b.is_some();
        let (peak, power, groups) = match b {
            Some(b) => {
                let deviation = b.mpx.params().deviation_hz;
                let peak = Hertz(f64::from(std::mem::take(&mut b.peak)) * deviation);
                (Some(peak), b.power_db(), b.mpx.groups())
            }
            None => (None, None, self.last_groups),
        };
        let f = FmTxFields {
            transmitting: on,
            tx_freq_hz: on.then_some(Hertz(self.dial)),
            tx_peak_hz: peak,
            tx_mpx_power_db: power,
            tx_groups: groups as i64,
        };
        self.results.put(&f);
    }

    /// One pass: the samples of the transmission, the end mark, and the mute state.
    fn run(&mut self, io: &mut Io) {
        let room = io.input_len(0);
        if self.rate > 0.0 && room > 0 {
            self.pass_s = room as f64 / self.rate;
        }
        if self.burst.is_some() {
            self.audio.take(io, 1);
        }
        let mut ended = std::mem::take(&mut self.end_owed);
        if ended {
            end_mark(io, 0);
        }
        if let Some(b) = &mut self.burst {
            b.fill(room, &mut self.audio);
            if b.out.write(io, 0, room, &self.timing) {
                self.last_groups = b.mpx.groups();
                self.burst = None;
                ended = true;
            }
        }
        let busy = self.burst.is_some();
        self.keyed.pass(io, 1, busy, ended, room, self.rate);
        if io.tick() || ended {
            self.publish();
        }
    }
}

impl Component for FmTransmit {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            ID,
            vec![
                Port::iq("in"),
                Port::audio("audio")
                    .channels(sdrkit_graph::ChannelRule::Exact(2))
                    .lossy()
                    .optional("silence: the pilot and RDS are sent alone"),
            ],
            vec![Port::iq("out"), KeyedOut::port()],
        )
        .with_settings(TxSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let f = &inputs[0];
        self.rate = f.rate;
        self.dial = if f.tuned_hz > 0.0 {
            f.tuned_hz
        } else {
            f.centre_hz
        };
        if let Some(a) = inputs.get(1) {
            self.audio.configure(a);
        }
        self.cut();
        self.keyed.reset();
        self.publish();
        Ok(vec![
            Format::iq(self.rate, self.dial, self.dial),
            KeyedOut::format(),
        ])
    }
    fn attach(&mut self, ctx: &RunContext) {
        self.transmit = ctx.transmit;
        self.timing.attach(ctx);
        self.audio.attach(ctx);
        self.wall = ctx.clock.clone();
        if !ctx.transmit {
            self.cut();
        }
    }
    fn process(&mut self, io: &mut Io) {
        self.run(io);
    }
    fn finish(&mut self, io: &mut Io) {
        self.run(io);
        self.cut();
        if std::mem::take(&mut self.end_owed) {
            end_mark(io, 0);
            self.publish();
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        let c = &mut self.cfg;
        let content = match TxSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            TxSetting::Pi => {
                c.pi = parse_pi(id, TX_PI.read(id, v)?)?;
                true
            }
            TxSetting::Ps => {
                c.ps = bounded(id, TX_PS.read(id, v)?, 8)?;
                true
            }
            TxSetting::Rt => {
                let rt = bounded(id, TX_RT.read(id, v)?, 64)?;
                if rt != c.rt {
                    c.rt = rt;
                    c.rt_ab = !c.rt_ab;
                }
                true
            }
            TxSetting::Pty => {
                c.pty = u8::try_from(TX_PTY.read(id, v)?)
                    .ok()
                    .filter(|p| *p <= 31)
                    .ok_or(SettingError::Malformed { id, at: None })?;
                true
            }
            TxSetting::Tp => {
                c.tp = TX_TP.read(id, v)?;
                true
            }
            TxSetting::Ta => {
                c.ta = TX_TA.read(id, v)?;
                true
            }
            TxSetting::Music => {
                c.music = TX_MUSIC.read(id, v)?;
                true
            }
            TxSetting::Af => {
                let t = TX_AF.read(id, v)?;
                c.af = parse_af(id, t)?;
                c.af_text = t.to_owned();
                true
            }
            TxSetting::Ct => {
                c.ct = TX_CT.read(id, v)?;
                true
            }
            TxSetting::CtOffset => {
                c.ct_offset = i8::try_from(TX_CT_OFFSET.read(id, v)?)
                    .ok()
                    .filter(|o| (-24..=24).contains(o))
                    .ok_or(SettingError::Malformed { id, at: None })?;
                true
            }
            TxSetting::Stereo => {
                c.stereo = TX_STEREO.read(id, v)?;
                false
            }
            TxSetting::Preemphasis => {
                c.preemphasis = TX_PREEMPHASIS.read(id, v)?;
                false
            }
            TxSetting::Deviation => {
                c.deviation = TX_DEVIATION.read(id, v)?;
                false
            }
            TxSetting::Pilot => {
                c.pilot_pct = TX_PILOT.read(id, v)?;
                false
            }
            TxSetting::RdsLevel => {
                c.rds_khz = TX_RDS_LEVEL.read(id, v)?;
                false
            }
            TxSetting::Limiter => {
                c.limiter = TX_LIMITER.read(id, v)?;
                false
            }
        };
        if content {
            self.content_changed();
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        let c = &self.cfg;
        sdrkit_graph::values_of([
            (
                TxSetting::Pi,
                Value::Text(c.pi.map(|p| format!("{p:04X}")).unwrap_or_default()),
            ),
            (TxSetting::Ps, Value::Text(c.ps.clone())),
            (TxSetting::Rt, Value::Text(c.rt.clone())),
            (TxSetting::Pty, Value::I64(i64::from(c.pty))),
            (TxSetting::Tp, Value::Bool(c.tp)),
            (TxSetting::Ta, Value::Bool(c.ta)),
            (TxSetting::Music, Value::Bool(c.music)),
            (TxSetting::Af, Value::Text(c.af_text.clone())),
            (TxSetting::Ct, Value::Bool(c.ct)),
            (TxSetting::CtOffset, Value::I64(i64::from(c.ct_offset))),
            (TxSetting::Stereo, Value::Bool(c.stereo)),
            (TxSetting::Preemphasis, Value::Choice(c.preemphasis.index())),
            (TxSetting::Deviation, Value::Hz(c.deviation)),
            (TxSetting::Pilot, Value::F64(c.pilot_pct)),
            (TxSetting::RdsLevel, Value::F64(c.rds_khz)),
            (TxSetting::Limiter, Value::Bool(c.limiter)),
        ])
    }
    fn results(&self) -> Option<&Results> {
        Some(self.results.results())
    }
    fn commands(&self) -> &Commands {
        TxCommand::commands()
    }
    fn invoke(&mut self, id: CommandId, _args: &Values) -> Result<Option<Record>, CommandError> {
        match TxCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            TxCommand::Start => self.start()?,
            TxCommand::Stop => self.stop(),
        }
        self.publish();
        Ok(None)
    }
    fn any_ref(&self) -> Option<&dyn std::any::Any> {
        Some(self)
    }
    fn any_mut(&mut self) -> Option<&mut dyn std::any::Any> {
        Some(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_codes_and_frequencies_parse() {
        let id = TxSetting::schema().id("tx_af").unwrap();
        assert_eq!(parse_af(id, "87.6 98.1, 107.9").unwrap(), vec![1, 106, 204]);
        assert!(parse_af(id, "87.5").is_err());
        assert!(parse_af(id, "98.15").is_err());
        assert_eq!(parse_pi(id, " 1234 ").unwrap(), Some(0x1234));
        assert_eq!(parse_pi(id, "").unwrap(), None);
        assert!(parse_pi(id, "0").is_err() && parse_pi(id, "12345").is_err());
        assert!(bounded(id, "ABCDEFGHI", 8).is_err());
    }
}
