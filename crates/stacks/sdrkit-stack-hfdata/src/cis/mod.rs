//! CIS HF data signals: seven signals in one stack, chosen by the submode. `bee` (CIS-36-50 and CIS-50-50, 2-FSK) reads the
//! message starts, the session key and the 3:4 characters as hex; `cis14` (CIS-14) shows its 14-bit frames raw; `cis12` (CIS-12)
//! reads the phase steps of its 12 DPSK carriers. `cis36` (CIS-36, 36-tone MFSK), `ofdm_hdr` (CIS-45, CIS-60, CIS-93), `cis128`
//! (CIS-128) and `cis3000` (CIS-3000, 8-PSK at 3000 Bd) are identified only. Traffic is encrypted; nothing is decrypted.
//!
//! The channel is the 24 kHz channel of the family front end decimated by 2 (a 63-tap half-band) to 12 000 samples/s, so the
//! receivers read 100 to 5900 Hz above the dial.
//!
//! Standard: no public document of an issuing body describes these signals.
//!
//! ```text
//! iq -> [hf_front 24 kHz] -> [cis_hf: half-band to 12 kHz, the receiver of the submode] -> text, messages, measures, identification
//! ```

pub mod cis12;
pub mod fsk;
pub mod gen;
pub mod mfsk36;
pub mod ofdm;

use crate::receiver::GEN_SECTION;
use crate::typed::{
    CisHfFields, CisHfMessageRow, CisIdentEvent, TextEvent, CIS_HF_CARRIERS, CIS_HF_MESSAGES,
    CIS_IDENT, CIS_MESSAGE, TEXT,
};
use sdrkit_algo_dsp::decim::FirDecimator;
use sdrkit_core::interface::{Fields as _, Hertz, Time};
use sdrkit_core::publish::{field, Published};
use sdrkit_core::{
    AutoGain, Choice, Clock, Complex32, ConfigError, LockProfile, LockState, Receiver, ResultsDecl,
    Schema, Setting, SettingError, SettingId, SettingSet, StackConfig, StackDescriptor, Status,
    SubMode, Value, Values,
};
use sdrkit_graph::{values_of, Format, Kind, Outbox, Port, RateFrom, RunContext};
use std::sync::Arc;

/// The rate of the channel.
pub const RATE: f64 = 12_000.0;
/// The text kept, characters.
const TEXT_KEEP: usize = 12_000;

sdrkit_core::choice_enum! {
    /// The signal.
    pub enum Submode {
        /// CIS-36-50 and CIS-50-50 (BEE): 2-FSK messages.
        Bee = ("bee", "CIS-36-50 / CIS-50-50 (BEE)"),
        /// CIS-14: 2-FSK frames.
        Cis14 = ("cis14", "CIS-14"),
        /// CIS-12: 12 DPSK carriers.
        Cis12 = ("cis12", "CIS-12"),
        /// CIS-36: 36-tone MFSK, identified only.
        Cis36 = ("cis36", "CIS-36 (identification)"),
        /// CIS-45, CIS-60, CIS-93: OFDM, identified only.
        OfdmHdr = ("ofdm_hdr", "CIS OFDM (identification)"),
        /// CIS-128: OFDM, identified only.
        Cis128 = ("cis128", "CIS-128 (identification)"),
        /// CIS-3000: 8-PSK at 3000 Bd, identified only.
        Cis3000 = ("cis3000", "CIS-3000 (identification)"),
    }
}

impl Submode {
    /// Whether the submode identifies the signal only.
    pub fn identify_only(self) -> bool {
        matches!(
            self,
            Submode::Cis36 | Submode::OfdmHdr | Submode::Cis128 | Submode::Cis3000
        )
    }
}

/// The submode.
pub const SUBMODE: Setting<field::Choice<Submode>> =
    Setting::choice("submode", "Signal", Submode::Bee);
/// The speed of `bee` and `cis14`; 0 tries the candidates.
pub const BAUD: Setting<field::F64> = Setting::new("baud", "Speed", "Bd", 0.0)
    .range(0.0, 300.0, 0.01)
    .help("bee and cis14: 0 tries every candidate speed; a speed replaces them.");
/// The shift of `bee` and `cis14`; 0 finds it.
pub const SHIFT: Setting<field::Hz> = Setting::new("shift_hz", "Shift", "Hz", 0.0)
    .range(0.0, 1200.0, 1.0)
    .help("bee and cis14: 0 finds it from the signal.");
/// The roll-off of the matched filter of `cis3000`.
pub const ROLL_OFF: Setting<field::F64> = Setting::new("roll_off", "CIS-3000 roll-off", "", 0.15)
    .range(0.05, 1.0, 0.01)
    .help(
        "The pulse of CIS-3000 is not published; a root-raised cosine of this roll-off is assumed.",
    )
    .advanced();

sdrkit_core::settings! {
    /// The settings of the CIS receiver.
    pub enum CisRxSetting {
        /// Signal.
        Submode = SUBMODE,
        /// Speed.
        Baud = BAUD,
        /// Shift.
        Shift = SHIFT,
        /// CIS-3000 roll-off.
        RollOff = ROLL_OFF,
    }
}

sdrkit_core::settings! {
    /// The settings of the CIS stack.
    pub enum CisSetting {
        /// Signal.
        Submode = SUBMODE,
        /// Speed.
        Baud = BAUD,
        /// Shift.
        Shift = SHIFT,
        /// CIS-3000 roll-off.
        RollOff = ROLL_OFF,
        /// BEE variant of the test signal.
        GenVariant = gen::GEN_VARIANT,
        /// BEE polarity of the test signal.
        GenLowerOne = gen::GEN_LOWER_ONE,
        /// Centre of the 2-FSK test signals.
        GenCentreHz = gen::GEN_CENTRE_HZ,
        /// Shift of the CIS-14 test signal.
        GenShiftHz = gen::GEN_SHIFT_HZ,
        /// PSK order of the CIS-12 test signal.
        GenOrder = gen::GEN_ORDER,
        /// Tone rate of the CIS-36 test signal.
        GenRate = gen::GEN_RATE,
        /// OFDM signal of the ofdm_hdr test signal.
        GenOfdm = gen::GEN_OFDM,
    }
}

/// What a submode measures: the fields of [`CisHfFields`] that depend on the submode. A field that the submode does
/// not measure stays `None`.
#[derive(Clone, Debug, Default)]
pub struct Measures<'a> {
    /// The centre of the signal above the dial.
    pub centre_hz: Option<f64>,
    /// The shift between the tones.
    pub shift_hz: Option<f64>,
    /// The eye opening.
    pub eye: Option<f64>,
    /// Frames read.
    pub frames: Option<i64>,
    /// The state bits, raw.
    pub state_bits: Option<&'a str>,
    /// The pilot.
    pub pilot_hz: Option<f64>,
    /// Carriers seen.
    pub carriers: Option<i64>,
    /// The symbol rate.
    pub symbol_rate: Option<f64>,
    /// The modulation.
    pub modulation: Option<&'a str>,
    /// The moments of the 2nd, 4th and 8th power.
    pub m2: Option<f64>,
    /// See `m2`.
    pub m4: Option<f64>,
    /// See `m2`.
    pub m8: Option<f64>,
    /// Tones seen.
    pub tones: Option<i64>,
    /// The spacing.
    pub spacing_hz: Option<f64>,
    /// The lowest tone or band edge.
    pub low_hz: Option<f64>,
    /// The highest tone or band edge.
    pub high_hz: Option<f64>,
    /// The span of the tones.
    pub span_hz: Option<f64>,
    /// Double gaps.
    pub double_gaps: Option<i64>,
    /// The useful part of a symbol, ms.
    pub useful_ms: Option<f64>,
    /// The guard, ms.
    pub guard_ms: Option<f64>,
    /// The centre carrier is off.
    pub centre_off: Option<bool>,
    /// A pilot at 3300 Hz.
    pub pilot_3300: Option<bool>,
    /// The lines of the 2nd, 4th and 8th power.
    pub line2_db: Option<f64>,
    /// See `line2_db`.
    pub line4_db: Option<f64>,
    /// See `line2_db`.
    pub line8_db: Option<f64>,
    /// The symbol-rate line.
    pub rate_line_db: Option<f64>,
}

/// What no public document confirms.
pub const NOTE_TEXT: &str = "No document of an issuing body describes these signals; the stacks follow public descriptions \
of hobby listeners. Not confirmed: the BEE start word bits (only its weight of 21 ones is used), the key sent twice, the \
3:4 characters and the end character 77, the shifts of the test signals; the CIS-14 parity rule, state codes and polarity \
(shown raw); the CIS-12 pulse shape and frames (the stack stops at the phase steps); the CIS-36 tone groups; the CIS-128 \
spacing and guard; the CIS-3000 pulse (a root-raised cosine, the setting roll_off). Four submodes identify the signal only.";

/// A `bee` message being read or read.
#[derive(Clone, Debug)]
pub struct Message {
    /// The UTC time of its start.
    pub time: f64,
    /// The speed.
    pub baud: f64,
    /// The start word, hex (empty for a follow-on message).
    pub start_word: String,
    /// The session key, hex.
    pub key: String,
    /// Characters.
    pub chars: i64,
    /// Characters failing the 3:4 check.
    pub bad: i64,
    /// How it ended.
    pub ended_by: String,
    /// The text of its start.
    pub text: String,
}

impl Message {
    fn new(time: f64, baud: f64) -> Self {
        Message {
            time,
            baud,
            start_word: String::new(),
            key: String::new(),
            chars: 0,
            bad: 0,
            ended_by: String::new(),
            text: String::new(),
        }
    }
}

/// What a receiver writes.
pub struct Out {
    /// The UTC time of the newest sample.
    pub time: f64,
    /// The dial, Hz.
    pub dial: f64,
    /// New text.
    pub chars: String,
    /// Events.
    pub events: Vec<sdrkit_core::Event>,
    /// Good characters, frames or symbols.
    pub good: i64,
    /// Bad ones.
    pub bad: i64,
    /// Messages that ended.
    pub messages: Vec<Message>,
}

/// The receiver of a submode.
pub trait Sub: Send {
    /// Takes samples at 12 kHz.
    fn push(&mut self, x: &[Complex32], out: &mut Out);
    /// 0 searching, 1 signal found, 2 decoding or identified.
    fn state(&self) -> u8;
    /// What was found, in one line.
    fn summary(&self) -> String;
    /// Fills what it measures.
    fn measure<'a>(&'a self, m: &mut Measures<'a>);
    /// Writes its own table rows.
    fn rows(&self, _res: &mut Published) {}
    /// The SNR, dB.
    fn snr_db(&self) -> Option<f64> {
        None
    }
    /// The frequency error, Hz.
    fn offset_hz(&self) -> Option<f64> {
        None
    }
    /// The name identified.
    fn identified(&self) -> Option<String> {
        None
    }
}

/// The taps of the 63-tap half-band low pass (Kaiser window, beta 7).
fn half_band() -> Vec<f32> {
    let n = 63;
    let w = sdrkit_algo_dsp::window::kaiser(n, 7.0);
    (0..n)
        .map(|i| {
            let k = i as f64 - 31.0;
            let s = if k == 0.0 {
                0.5
            } else {
                (std::f64::consts::FRAC_PI_2 * k).sin() / (std::f64::consts::PI * k)
            };
            (s * w[i]) as f32
        })
        .collect()
}

/// The CIS decoder.
pub struct CisDecoder {
    submode: Submode,
    baud: f64,
    shift: f64,
    roll_off: f64,
    decim: FirDecimator<Complex32>,
    sub: Box<dyn Sub>,
    n: u64,
    buf: Vec<Complex32>,
    text: String,
    line: String,
    good: i64,
    bad: i64,
    messages: Vec<Message>,
    new_rows: usize,
    serial: u64,
    cleared: bool,
    ident: Option<String>,
    clock: Option<Clock>,
    utc0: Option<f64>,
    dial: f64,
    status: Status,
    out: Out,
}

impl Default for CisDecoder {
    fn default() -> Self {
        Self::new()
    }
}

impl CisDecoder {
    /// A receiver at the default settings.
    pub fn new() -> Self {
        let mut d = CisDecoder {
            submode: Submode::Bee,
            baud: 0.0,
            shift: 0.0,
            roll_off: 0.15,
            decim: FirDecimator::new(&half_band(), 2),
            sub: Box::new(fsk::Bee::new(0.0, 0.0)),
            n: 0,
            buf: Vec::new(),
            text: String::new(),
            line: String::new(),
            good: 0,
            bad: 0,
            messages: Vec::new(),
            new_rows: 0,
            serial: 0,
            cleared: false,
            ident: None,
            clock: None,
            utc0: None,
            dial: 0.0,
            status: Status::default(),
            out: Out {
                time: 0.0,
                dial: 0.0,
                chars: String::new(),
                events: Vec::new(),
                good: 0,
                bad: 0,
                messages: Vec::new(),
            },
        };
        d.rebuild();
        d
    }

    fn rebuild(&mut self) {
        self.decim.reset();
        self.ident = None;
        self.sub = match self.submode {
            Submode::Bee => Box::new(fsk::Bee::new(self.baud, self.shift)),
            Submode::Cis14 => Box::new(fsk::Cis14::new(self.baud, self.shift)),
            Submode::Cis12 => Box::new(cis12::Cis12::new()),
            Submode::Cis36 => Box::new(mfsk36::Cis36::new()),
            Submode::OfdmHdr => Box::new(ofdm::Ofdm::new(false)),
            Submode::Cis128 => Box::new(ofdm::Ofdm::new(true)),
            Submode::Cis3000 => Box::new(ofdm::Cis3000::new(self.roll_off)),
        };
    }

    fn flush(&mut self, out: &mut Outbox) {
        for e in self.out.events.drain(..) {
            out.messages(0).push(e);
        }
        self.good += self.out.good;
        self.bad += self.out.bad;
        self.out.good = 0;
        self.out.bad = 0;
        for m in self.out.messages.drain(..) {
            self.messages.push(m);
            self.new_rows += 1;
            if self.messages.len() > 200 {
                self.messages.remove(0);
            }
        }
        let chars = std::mem::take(&mut self.out.chars);
        for c in chars.chars() {
            self.text.push(c);
            if c == '\n' {
                if !self.line.trim().is_empty() {
                    out.messages(0).push(TEXT.event(
                        self.out.time,
                        self.dial,
                        &TextEvent {
                            text: self.line.trim(),
                        },
                    ));
                }
                self.line.clear();
            } else {
                self.line.push(c);
                if self.line.len() >= 96 {
                    out.messages(0).push(TEXT.event(
                        self.out.time,
                        self.dial,
                        &TextEvent {
                            text: self.line.trim(),
                        },
                    ));
                    self.line.clear();
                }
            }
        }
        if self.text.len() > TEXT_KEEP + 2000 {
            let cut = self.text.len() - TEXT_KEEP;
            let cut = (cut..self.text.len())
                .find(|&i| self.text.is_char_boundary(i))
                .unwrap_or(0);
            self.text.drain(..cut);
        }
        let id = self.sub.identified();
        if id.is_some() && id != self.ident {
            if let Some(name) = &id {
                out.messages(0).push(CIS_IDENT.event(
                    self.out.time,
                    self.dial,
                    &CisIdentEvent {
                        text: &self.sub.summary(),
                        identified: name,
                    },
                ));
            }
        }
        if id.is_some() {
            self.ident = id;
        }
    }
}

/// The results.
pub fn declaration() -> ResultsDecl {
    ResultsDecl::new()
        .with_fields(CisHfFields::SHAPE.fields)
        .with_table(&CIS_HF_MESSAGES)
        .with_table(&CIS_HF_CARRIERS)
        .event(&TEXT)
        .event(&CIS_MESSAGE)
        .event(&CIS_IDENT)
}

static COMMANDS: [&sdrkit_core::command::Command; 1] = [&crate::teleprinter::RESET];

impl sdrkit_graph::Decoder for CisDecoder {
    type Input = Complex32;
    fn name(&self) -> &'static str {
        "hfdata.cis_hf"
    }
    fn outputs(&self) -> Vec<Port> {
        vec![Port::new("events", Kind::Messages).rate_from(RateFrom::Fixed(0.0))]
    }
    fn settings(&self) -> Arc<Schema> {
        CisRxSetting::schema()
    }
    fn values(&self) -> Values {
        values_of([
            (CisRxSetting::Submode, Value::Choice(self.submode.index())),
            (CisRxSetting::Baud, Value::F64(self.baud)),
            (CisRxSetting::Shift, Value::Hz(self.shift)),
            (CisRxSetting::RollOff, Value::F64(self.roll_off)),
        ])
    }
    fn apply(&mut self, id: SettingId, value: &Value) -> Result<(), SettingError> {
        match CisRxSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            CisRxSetting::Submode => self.submode = SUBMODE.read(id, value)?,
            CisRxSetting::Baud => self.baud = BAUD.read(id, value)?,
            CisRxSetting::Shift => self.shift = SHIFT.read(id, value)?,
            CisRxSetting::RollOff => self.roll_off = ROLL_OFF.read(id, value)?,
        }
        self.rebuild();
        Ok(())
    }
    fn commands(&self) -> &'static [&'static sdrkit_core::command::Command] {
        &COMMANDS
    }
    fn invoke(
        &mut self,
        id: sdrkit_core::command::CommandId,
        _args: &Values,
    ) -> Result<Option<sdrkit_core::Record>, sdrkit_core::command::CommandError> {
        if id.index() == 0 {
            self.rebuild();
            Ok(None)
        } else {
            Err(sdrkit_core::command::CommandError::NoSuchId { id })
        }
    }
    fn results(&self) -> Published {
        Published::from_decl(Arc::new(declaration()))
    }
    fn attach(&mut self, ctx: &RunContext) {
        self.clock = Some(ctx.clock.clone());
    }
    fn configure(&mut self, input: &Format) -> Result<Vec<Format>, ConfigError> {
        crate::serial::check_rate(input)?;
        self.dial = input.tuned_hz;
        self.rebuild();
        Ok(vec![Format::new(Kind::Messages, 0.0)])
    }
    fn retune(&mut self, input: &Format) {
        self.dial = input.tuned_hz;
        self.rebuild();
    }
    fn process(&mut self, x: &[Complex32], _first: u64, out: &mut Outbox) {
        if self.utc0.is_none() {
            let now = self.clock.as_ref().map_or(0.0, Clock::now);
            self.utc0 = Some(now - self.n as f64 / crate::teleprinter::RATE_IN);
        }
        self.out.dial = self.dial;
        for &v in x {
            let v = if v.re.is_finite() && v.im.is_finite() {
                v
            } else {
                Complex32::default()
            };
            self.n += 1;
            if let Some(y) = self.decim.push(v) {
                self.buf.push(y);
                // Pieces of 120 samples (10 ms) at fixed sample counts, so the cut of the input does not matter.
                if self.buf.len() == 120 {
                    self.out.time =
                        self.utc0.unwrap_or(0.0) + self.n as f64 / crate::teleprinter::RATE_IN;
                    let b = std::mem::take(&mut self.buf);
                    self.sub.push(&b, &mut self.out);
                    self.buf = b;
                    self.buf.clear();
                    self.flush(out);
                }
            }
        }
    }
    fn report(&mut self, res: &mut Published) {
        if std::mem::take(&mut self.cleared) {
            res.clear_rows(&CIS_HF_MESSAGES);
        }
        let start = self.messages.len().saturating_sub(self.new_rows);
        for (k, m) in self.messages.iter().skip(start).enumerate() {
            let key = format!("{}", self.serial + k as u64);
            CIS_HF_MESSAGES.put(
                res,
                &key,
                &CisHfMessageRow {
                    time: Time(m.time),
                    speed: m.baud,
                    start_word: &m.start_word,
                    session_key: &m.key,
                    characters: m.chars,
                    failing: m.bad,
                    ended_by: &m.ended_by,
                },
            );
        }
        self.serial += self.new_rows as u64;
        self.new_rows = 0;
        self.sub.rows(res);
        let state = self.sub.state();
        let summary = self.sub.summary();
        let snr = self.sub.snr_db().filter(|v| v.is_finite());
        let off = self.sub.offset_hz().filter(|v| v.is_finite());
        let mut m = Measures::default();
        self.sub.measure(&mut m);
        res.put(&CisHfFields {
            note: NOTE_TEXT,
            text: &self.text,
            state: i64::from(state),
            found: &summary,
            identified: self.ident.as_deref().unwrap_or(""),
            good: self.good,
            bad: self.bad,
            offset_hz: off.map(Hertz),
            snr_db: snr.map(|v| (v * 10.0).round() / 10.0),
            centre_hz: m.centre_hz,
            shift_hz: m.shift_hz,
            eye: m.eye,
            frames: m.frames,
            state_bits: m.state_bits,
            pilot_hz: m.pilot_hz,
            carriers: m.carriers,
            symbol_rate: m.symbol_rate,
            modulation: m.modulation,
            m2: m.m2,
            m4: m.m4,
            m8: m.m8,
            tones: m.tones,
            spacing_hz: m.spacing_hz,
            low_hz: m.low_hz,
            high_hz: m.high_hz,
            span_hz: m.span_hz,
            double_gaps: m.double_gaps,
            useful_ms: m.useful_ms,
            guard_ms: m.guard_ms,
            centre_off: m.centre_off,
            pilot_3300: m.pilot_3300,
            line2_db: m.line2_db,
            line4_db: m.line4_db,
            line8_db: m.line8_db,
            rate_line_db: m.rate_line_db,
        });
        self.status.lock_state = match state {
            2 => LockState::Locked,
            1 => LockState::Tracking,
            _ => LockState::Searching,
        };
        self.status.snr_db = snr.map(|v| v as f32);
        self.status.offset_hz = off;
        self.status.summary = if self.submode.identify_only() {
            format!("identification only: {summary}")
        } else {
            summary
        };
        res.results_mut().set_status(&self.status);
    }
    fn clear(&mut self) {
        self.text.clear();
        self.line.clear();
        self.good = 0;
        self.bad = 0;
        self.messages.clear();
        self.new_rows = 0;
        self.cleared = true;
    }
}

/// The submode of a configuration: the submode named, else the `submode` setting.
pub fn submode_of(sub: Option<&str>, settings: &Values) -> Submode {
    sub.and_then(Submode::from_key)
        .unwrap_or_else(|| settings.get(&SUBMODE))
}

/// The settings of the stack.
pub fn schema() -> Arc<Schema> {
    CisSetting::schema()
}

/// `cfg` with its submode written into the `submode` setting.
fn with_submode(cfg: &StackConfig) -> Result<StackConfig, ConfigError> {
    let mut cfg = cfg.clone();
    if let Some(sub) = cfg.submode.as_deref().and_then(Submode::from_key) {
        if let Ok(id) = schema().id(SUBMODE.field.key) {
            cfg.settings.set(id, &Value::Choice(sub.index()))?;
        }
    }
    Ok(cfg)
}

fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    crate::receiver::decoder_receiver(
        crate::typed::shape("cis_hf"),
        "CIS HF",
        CisDecoder::new(),
        &with_submode(cfg)?,
    )
}

/// The dial of the test signals. It is not a station frequency: the host gives the frequency.
pub const TEST_SIGNAL_HZ: f64 = 6.5e6;

const fn sub(id: &'static str, name: &'static str) -> SubMode {
    SubMode {
        id,
        name,
        default_hz: TEST_SIGNAL_HZ,
    }
}

/// CIS HF data signals.
pub static CIS_HF: StackDescriptor = StackDescriptor {
    id: "cis_hf",
    name: "CIS HF data signals",
    group: "HF utility",
    blurb: "CIS HF signals: BEE messages (2-FSK) as hex, CIS-14 frames raw, the CIS-12 phase steps; CIS-36, the CIS OFDM signals, CIS-128 and CIS-3000 identified only.",
    tuning: crate::ms110::hf_tuning(TEST_SIGNAL_HZ),
    submodes: &[
        sub("bee", "CIS-36-50 / CIS-50-50 (BEE)"),
        sub("cis14", "CIS-14"),
        sub("cis12", "CIS-12"),
        sub("cis36", "CIS-36 (identification)"),
        sub("ofdm_hdr", "CIS OFDM (identification)"),
        sub("cis128", "CIS-128 (identification)"),
        sub("cis3000", "CIS-3000 (identification)"),
    ],
    settings: schema,
    make_receiver,
    make_generator: Some(gen::generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[],
    auto_gain: AutoGain::Radio,
    verify: Some(&crate::verify::CIS_HF),
    benchmarks: Some(crate::benchmarks),
};

/// The section of the test-signal settings.
pub(crate) const GEN: &str = GEN_SECTION;
