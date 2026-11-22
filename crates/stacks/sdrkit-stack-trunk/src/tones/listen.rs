//! `tones.gate`: the gate of the listening audio. It opens always, on any CTCSS tone or DCS code, or on the chosen one only,
//! from the `tones.subaudio` events.

use crate::{typed_format, typed_port};
use sdrkit_algo_dsp::iir::{Biquad, BiquadCoeffs};
use sdrkit_core::publish::field;
use sdrkit_core::settings::CatalogueSetting;
use sdrkit_core::{
    Choice, ChoiceDecl, ConfigError, Setting, SettingError, SettingId, SettingSet, Value, Values,
};
use sdrkit_graph::{
    values_of, Applied, Component, ComponentInfo, Format, Io, Item, Kind, Port, Timed,
};
use sdrkit_proto_tones::components::SubAudioEvent;
use sdrkit_proto_tones::ctcss::CtcssEventKind;
use sdrkit_proto_tones::dcs::DcsEventKind;
use sdrkit_util_ids::tones::ctcss::TONES;
use sdrkit_util_ids::tones::dcs::{Dcs, STANDARD};
use std::collections::VecDeque;

/// The audio rate of the gate.
const RATE: f64 = 8000.0;
/// The time an opening is held after its condition goes (0.4 s).
const HOLD: u64 = 3200;

/// The gate's state, reported with each piece of audio.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct GateState {
    /// Open now.
    pub open: bool,
}

impl Item for GateState {
    const KIND: &'static str = "tones.gate";
}

sdrkit_core::choice_enum! {
    /// When the gate opens.
    pub enum GateMode {
        /// Always open.
        Always = ("always", "always"),
        /// Open on any CTCSS tone or DCS code.
        Any = ("any", "on any tone or code"),
        /// Open only on the chosen tone or code.
        Chosen = ("chosen", "only on the chosen tone or code"),
    }
}

sdrkit_core::choice_enum! {
    /// What the gate opens on.
    pub enum GateKind {
        /// A CTCSS tone.
        Ctcss = ("ctcss", "CTCSS"),
        /// A DCS code.
        Dcs = ("dcs", "DCS"),
    }
}

/// What the sub-audio decoders hold now.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Held {
    ctcss: Option<f32>,
    dcs: Option<(Dcs, bool)>,
}

/// `tones.gate`: the gate and the listening audio (a 300 Hz high-pass that takes the sub-audible tones out, a gain that moves
/// by 0.01 for each sample).
pub struct GateComponent {
    mode: GateMode,
    kind: GateKind,
    tone: usize,
    code: usize,
    inverted: bool,
    listen: bool,
    hp: Biquad,
    gain: f32,
    held: Held,
    open_until: u64,
    pending: VecDeque<(u64, SubAudioEvent)>,
}

impl Default for GateComponent {
    fn default() -> Self {
        GateComponent {
            mode: GateMode::Always,
            kind: GateKind::Ctcss,
            tone: usize::from(GATE_TONE.default_index()),
            code: usize::from(GATE_CODE.default_index()),
            inverted: false,
            listen: false,
            hp: Biquad::new(BiquadCoeffs::highpass(
                RATE,
                300.0,
                std::f64::consts::FRAC_1_SQRT_2,
            )),
            gain: 0.0,
            held: Held::default(),
            open_until: 0,
            pending: VecDeque::with_capacity(16),
        }
    }
}

/// The CTCSS tones as choices ("123.0").
fn tone_choices() -> Vec<ChoiceDecl> {
    TONES
        .iter()
        .map(|t| ChoiceDecl {
            key: format!("{:.1}", t.hz).into(),
            label: format!("{:.1} Hz", t.hz).into(),
        })
        .collect()
}

/// The standard DCS codes as choices ("023").
fn code_choices() -> Vec<ChoiceDecl> {
    STANDARD
        .iter()
        .map(|c| ChoiceDecl {
            key: format!("{c:03o}").into(),
            label: format!("D{c:03o}").into(),
        })
        .collect()
}

/// When the gate opens.
pub const GATE: Setting<field::Choice<GateMode>> =
    Setting::choice("gate", "Gate", GateMode::Always);
/// What the gate opens on.
pub const GATE_KIND: Setting<field::Choice<GateKind>> =
    Setting::choice("gate_kind", "Gate on", GateKind::Ctcss);
/// The chosen CTCSS tone.
pub const GATE_TONE: CatalogueSetting = CatalogueSetting {
    key: "gate_tone",
    label: "Tone",
    help: "The CTCSS tone that opens the gate.",
    choices: tone_choices,
    default: "100.0",
    advanced: false,
    role: None,
};
/// The chosen DCS code.
pub const GATE_CODE: CatalogueSetting = CatalogueSetting {
    key: "gate_code",
    label: "Code",
    help: "The DCS code that opens the gate.",
    choices: code_choices,
    default: "023",
    advanced: false,
    role: None,
};
/// The chosen DCS code is inverted.
pub const GATE_INVERTED: Setting<field::Bool> =
    Setting::new("gate_inverted", "Code inverted", "", false);
/// Give the listening audio.
pub const LISTEN: Setting<field::Bool> = Setting::new("listen", "Listen", "", false);

sdrkit_core::settings! {
    /// The settings of `tones.gate`.
    pub enum GateSetting {
        /// When the gate opens.
        Gate = GATE,
        /// What it opens on.
        GateKind = GATE_KIND,
        /// The chosen tone.
        GateTone = GATE_TONE,
        /// The chosen code.
        GateCode = GATE_CODE,
        /// The chosen code is inverted.
        GateInverted = GATE_INVERTED,
        /// Give the listening audio.
        Listen = LISTEN,
    }
}

impl GateComponent {
    fn condition(&self) -> bool {
        match self.mode {
            GateMode::Always => true,
            GateMode::Any => self.held.ctcss.is_some() || self.held.dcs.is_some(),
            GateMode::Chosen => match self.kind {
                GateKind::Dcs => self.held.dcs.is_some_and(|(c, inv)| {
                    STANDARD.get(self.code) == Some(&c.value()) && inv == self.inverted
                }),
                GateKind::Ctcss => self.held.ctcss.is_some_and(|t| {
                    TONES
                        .get(self.tone)
                        .is_some_and(|c| (t - c.hz).abs() < 0.05)
                }),
            },
        }
    }
    /// Takes one event of the sub-audio decoders. A turn-off closes the gate at once.
    fn take(&mut self, e: &SubAudioEvent, n: u64) {
        match e {
            SubAudioEvent::Ctcss(c) => match c.kind {
                CtcssEventKind::Locked => self.held.ctcss = Some(c.tone.hz),
                CtcssEventKind::Released => self.held.ctcss = None,
                CtcssEventKind::TurnOff => {
                    self.held.ctcss = None;
                    self.open_until = n;
                }
            },
            SubAudioEvent::Dcs(d) => match d.kind {
                DcsEventKind::Locked => self.held.dcs = Some((d.reading.code, d.reading.inverted)),
                DcsEventKind::Released => self.held.dcs = None,
                DcsEventKind::TurnOff => {
                    self.held.dcs = None;
                    self.open_until = n;
                }
            },
        }
    }
    /// Open now (at input sample `n`).
    fn open(&self, n: u64) -> bool {
        self.mode == GateMode::Always || n < self.open_until
    }
}

impl Component for GateComponent {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "tones.gate",
            vec![Port::real("in"), Port::typed::<SubAudioEvent>("sub")],
            vec![
                Port::audio("out").same_format_as(0),
                typed_port::<GateState>("state"),
            ],
        )
        .with_settings(GateSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.hp.reset();
        self.gain = 0.0;
        self.held = Held::default();
        self.open_until = 0;
        self.pending.clear();
        Ok(vec![
            inputs[0].with_kind(Kind::Audio),
            typed_format::<GateState>(&inputs[0]),
        ])
    }
    fn process(&mut self, io: &mut Io) {
        for s in io.items::<SubAudioEvent>(1) {
            self.pending
                .push_back(((s.time * RATE).round() as u64, s.value));
        }
        let first = io.input(0).first();
        let x = io.real(0);
        let mut out = std::mem::take(&mut io.audio_out(0).samples);
        for (k, &v) in x.iter().enumerate() {
            let n = first + k as u64;
            while let Some((at, e)) = self.pending.front().copied() {
                if at > n {
                    break;
                }
                self.pending.pop_front();
                self.take(&e, n);
            }
            if self.condition() && !matches!(self.mode, GateMode::Always) {
                self.open_until = n + HOLD;
            }
            let target = if self.open(n) { 1.0 } else { 0.0 };
            self.gain += (target - self.gain).clamp(-0.01, 0.01);
            let y = self.hp.process_sample(v) * self.gain;
            if self.listen {
                out.push(y);
            }
        }
        let end = first + x.len() as u64;
        io.audio_out(0).samples = out;
        io.items_out::<GateState>(1).push(Timed {
            time: end as f64 / RATE,
            value: GateState {
                open: self.open(end),
            },
        });
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match GateSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            GateSetting::Gate => self.mode = GATE.read(id, v)?,
            GateSetting::GateKind => self.kind = GATE_KIND.read(id, v)?,
            GateSetting::GateTone => self.tone = GATE_TONE.read(id, v)?,
            GateSetting::GateCode => self.code = GATE_CODE.read(id, v)?,
            GateSetting::GateInverted => self.inverted = GATE_INVERTED.read(id, v)?,
            GateSetting::Listen => self.listen = LISTEN.read(id, v)?,
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        let index = |i: usize| Value::Choice(u16::try_from(i).unwrap_or(u16::MAX));
        values_of([
            (GateSetting::Gate, Value::Choice(self.mode.index())),
            (GateSetting::GateKind, Value::Choice(self.kind.index())),
            (GateSetting::GateTone, index(self.tone)),
            (GateSetting::GateCode, index(self.code)),
            (GateSetting::GateInverted, Value::Bool(self.inverted)),
            (GateSetting::Listen, Value::Bool(self.listen)),
        ])
    }
}
