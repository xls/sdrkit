//! The tone identification of the general receiver: which CTCSS tone or DCS code is on the channel, and the tone gate that opens
//! the audio only on a chosen tone or code. The decoding is `tones.subaudio` of `sdrkit-proto-tones`.

use crate::listen::settings::parse_dcs;
use crate::types::RestartCommand;
use sdrkit_core::publish::field;
use sdrkit_core::{
    CommandError, CommandId, CommandSet, Commands, ConfigError, Record, Setting, SettingError,
    SettingId, SettingSet, Value, Values,
};
use sdrkit_graph::{
    values_of, Applied, Component, ComponentInfo, Format, Io, Item, Kept, Kind, Port, RateFrom,
    Timed,
};
use sdrkit_proto_tones::components::SubAudioEvent;
use sdrkit_proto_tones::ctcss::CtcssEventKind;
use sdrkit_proto_tones::dcs::DcsEventKind;
use sdrkit_util_ids::tones::dcs::{Dcs, Reading};

/// The level of a sine at 5 kHz deviation in the NFM audio (0.5 of full scale), dB relative to a full-scale sine.
const FIVE_KHZ_DB: f32 = -6.020_6;

/// What the tone identification found.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ToneId {
    /// The locked CTCSS tone (Hz, from the table).
    pub ctcss_hz: Option<f64>,
    /// Its measured frequency (Hz).
    pub ctcss_exact_hz: f64,
    /// Its level in dB against 5 kHz deviation.
    pub ctcss_db: f32,
    /// The locked DCS code, its polarity and another code of the standard list that the waveform also reads as.
    pub dcs: Option<Reading>,
    /// Whether the tone gate lets the audio through now.
    pub gate_open: bool,
}

impl Default for ToneId {
    fn default() -> Self {
        ToneId {
            ctcss_hz: None,
            ctcss_exact_hz: 0.0,
            ctcss_db: 0.0,
            dcs: None,
            gate_open: true,
        }
    }
}

impl Item for ToneId {
    const KIND: &'static str = "general.tone";
}

/// The gate modes of the tone identification.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum ToneGate {
    /// Always open.
    #[default]
    Open,
    /// Open on any tone or code.
    Any,
    /// Open only on this tone (within 1.2 Hz).
    Tone(f64),
    /// Open only on this code and polarity (`true` is inverted).
    Code(Dcs, bool),
}

impl ToneGate {
    /// Whether the gate is open for the tone `ctcss_hz` and the code `dcs` found now.
    pub fn open(&self, ctcss_hz: Option<f64>, dcs: Option<(Dcs, bool)>) -> bool {
        match *self {
            ToneGate::Open => true,
            ToneGate::Any => ctcss_hz.is_some() || dcs.is_some(),
            ToneGate::Tone(t) => ctcss_hz.is_some_and(|c| (c - t).abs() <= 1.2),
            ToneGate::Code(c, inv) => dcs == Some((c, inv)),
        }
    }
}

/// Reads a gate mode: `open` (or empty), `any`, `tone:88.5`, or `code:023N` (`D023N`, `023I` for the inverted polarity).
///
/// # Errors
///
/// The byte position in `s` of the part that does not parse.
pub fn parse_gate(s: &str) -> Result<ToneGate, usize> {
    let lead = s.len() - s.trim_start().len();
    let t = s.trim();
    match t {
        "open" | "" => Ok(ToneGate::Open),
        "any" => Ok(ToneGate::Any),
        _ => {
            let at = lead + 5;
            if let Some(f) = t.strip_prefix("tone:") {
                let hz = f.trim().parse::<f64>().map_err(|_| at)?;
                if hz.is_finite() && hz > 0.0 {
                    Ok(ToneGate::Tone(hz))
                } else {
                    Err(at)
                }
            } else {
                let c = t.strip_prefix("code:").ok_or(lead)?.trim();
                let inv = c.ends_with(['I', 'i']);
                let digits = c.trim_end_matches(['N', 'n', 'I', 'i']);
                Ok(ToneGate::Code(parse_dcs(digits).ok_or(at)?, inv))
            }
        }
    }
}

const ENABLED: Setting<field::Bool> = Setting::new("enabled", "Tone identification", "", false);
const GATE: Setting<field::Text> =
    Setting::new("gate", "Tone gate", "", "open").help("open, any, tone:<Hz> or code:<023N>.");

sdrkit_core::settings! {
    /// The settings of [`ToneIdComponent`].
    pub enum ToneIdSetting {
        /// The identification works. Off, it reports nothing and the gate is open.
        Enabled = ENABLED,
        /// The tone gate, as [`parse_gate`] reads it.
        Gate = GATE,
    }
}

/// The tone identification component (`general.toneid`): the audio that the decoders read (`audio`, for the time) and the
/// `tones.subaudio` items (`in`) in, the findings ([`ToneId`]) out at each tick of the graph clock. Its command `restart` forgets
/// what was found.
pub struct ToneIdComponent {
    enabled: bool,
    gate: ToneGate,
    gate_text: String,
    id: ToneId,
    dcs: Option<(Dcs, bool)>,
    rate: f64,
}

impl Default for ToneIdComponent {
    fn default() -> Self {
        ToneIdComponent {
            enabled: false,
            gate: ToneGate::Open,
            gate_text: "open".into(),
            id: ToneId::default(),
            dcs: None,
            rate: 48e3,
        }
    }
}

impl ToneIdComponent {
    fn drop_tone(&mut self) {
        self.id = ToneId::default();
        self.dcs = None;
    }
    fn take(&mut self, e: &SubAudioEvent) {
        match e {
            SubAudioEvent::Ctcss(c) => match c.kind {
                CtcssEventKind::Locked => {
                    self.id.ctcss_hz = Some(c.tone.hz as f64);
                    self.id.ctcss_exact_hz = c.measured_hz as f64;
                    self.id.ctcss_db = c.level_db - FIVE_KHZ_DB;
                }
                CtcssEventKind::Released | CtcssEventKind::TurnOff => self.id.ctcss_hz = None,
            },
            SubAudioEvent::Dcs(d) => match d.kind {
                DcsEventKind::Locked => {
                    let r = d.reading;
                    self.dcs = Some((r.code, r.inverted));
                    self.id.dcs = Some(r);
                }
                DcsEventKind::Released | DcsEventKind::TurnOff => {
                    self.dcs = None;
                    self.id.dcs = None;
                }
            },
        }
    }
}

impl Component for ToneIdComponent {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "general.toneid",
            vec![Port::real("audio"), Port::typed::<SubAudioEvent>("in")],
            vec![Port::typed::<ToneId>("tone").rate_from(RateFrom::Fixed(0.0))],
        )
        .with_settings(ToneIdSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.rate = inputs[0].rate;
        self.drop_tone();
        Ok(vec![Format::new(Kind::typed::<ToneId>(), 0.0)])
    }
    fn process(&mut self, io: &mut Io) {
        if self.enabled {
            let n = io.items::<SubAudioEvent>(1).len();
            for i in 0..n {
                let e = io.items::<SubAudioEvent>(1)[i].value;
                self.take(&e);
            }
        }
        if io.tick() {
            self.id.gate_open = !self.enabled || self.gate.open(self.id.ctcss_hz, self.dcs);
            let time = io.input(0).end() as f64 / self.rate;
            io.items_out::<ToneId>(0).push(Timed {
                time,
                value: self.id,
            });
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match ToneIdSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            ToneIdSetting::Enabled => {
                self.enabled = ENABLED.read(id, v)?;
                if !self.enabled {
                    self.drop_tone();
                }
            }
            ToneIdSetting::Gate => {
                let t = GATE.read(id, v)?;
                self.gate =
                    parse_gate(t).map_err(|at| SettingError::Malformed { id, at: Some(at) })?;
                t.clone_into(&mut self.gate_text);
            }
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([
            (ToneIdSetting::Enabled, Value::Bool(self.enabled)),
            (ToneIdSetting::Gate, Value::Text(self.gate_text.clone())),
        ])
    }
    /// Forgets the tone of the frequency before.
    fn forget(&mut self, _kept: &Kept) {
        self.drop_tone();
    }
    fn commands(&self) -> &Commands {
        RestartCommand::commands()
    }
    fn invoke(&mut self, id: CommandId, _args: &Values) -> Result<Option<Record>, CommandError> {
        match RestartCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            RestartCommand::Restart => self.drop_tone(),
        }
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gate_text_is_read_with_the_place_of_a_fault() {
        assert_eq!(parse_gate("open"), Ok(ToneGate::Open));
        assert_eq!(parse_gate(" any"), Ok(ToneGate::Any));
        assert_eq!(parse_gate("tone:88.5"), Ok(ToneGate::Tone(88.5)));
        assert!(matches!(
            parse_gate("code:023I"),
            Ok(ToneGate::Code(_, true))
        ));
        assert_eq!(parse_gate("tone:x"), Err(5));
        assert_eq!(parse_gate("loud"), Err(0));
    }
}
