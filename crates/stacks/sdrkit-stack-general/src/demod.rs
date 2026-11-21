//! The demodulator component of the general receiver: FM (NFM, WFM mono), AM envelope, SSB and CW with a BFO, on the shared
//! analogue demodulator.

use crate::types::{Demod, DemodReport, NfmLowPass, RestartCommand, AUDIO_RATE};
use sdrkit_algo_modem::analog::{
    AgcConfig, AnalogConfig, AnalogDemod, AnalogMode, AudioFilter, Gain, LowPass,
};
use sdrkit_algo_modem::ModemError;
use sdrkit_core::publish::field;
use sdrkit_core::{
    Choice, CommandError, CommandId, CommandSet, Commands, ConfigError, Record, Setting,
    SettingError, SettingId, SettingSet, StageError, Value, Values,
};
use sdrkit_graph::{
    values_of, Applied, Component, ComponentInfo, Format, Io, Kind, Port, RateFrom, Timed,
};

/// A gain in dB as a linear amplitude.
fn db_to_amp(db: f64) -> f32 {
    10f64.powf(db / 20.0) as f32
}

/// The SSB and CW AGC: attack 0.05 per sample at 48 kHz, decay 0.5 s, the envelope held at 0.5.
fn ssb_agc() -> AgcConfig {
    AgcConfig {
        target: 0.5,
        attack_s: -1.0 / (AUDIO_RATE * 0.95f64.ln()),
        decay_s: 0.5,
        max_gain: 1e4,
    }
}

/// The analogue mode of a demodulator choice.
fn mode_of(d: Demod) -> AnalogMode {
    match d {
        Demod::Nfm => AnalogMode::Nfm,
        Demod::Wfm => AnalogMode::Wfm,
        Demod::Am => AnalogMode::Am,
        Demod::Usb => AnalogMode::Usb,
        Demod::Lsb => AnalogMode::Lsb,
        Demod::Cw => AnalogMode::Cw,
    }
}

/// The demodulator choice of an analogue mode.
fn demod_of(m: AnalogMode) -> Demod {
    match m {
        AnalogMode::Nfm => Demod::Nfm,
        AnalogMode::Wfm => Demod::Wfm,
        AnalogMode::Am => Demod::Am,
        AnalogMode::Usb => Demod::Usb,
        AnalogMode::Lsb => Demod::Lsb,
        AnalogMode::Cw => Demod::Cw,
    }
}

/// The gain stage and the AM normalisation for the AGC flag and the manual gain: FM has none, AM with AGC divides by the carrier,
/// SSB and CW with AGC follow the envelope.
fn gain_of(d: Demod, agc: bool, gain: f32) -> (Gain, bool) {
    match d {
        Demod::Nfm | Demod::Wfm => (Gain::Manual(1.0), false),
        Demod::Am if agc => (Gain::Manual(1.0), true),
        Demod::Usb | Demod::Lsb | Demod::Cw if agc => (Gain::Auto(ssb_agc()), false),
        _ => (Gain::Manual(gain), false),
    }
}

/// The configuration of `demod` at `rate` (48 kHz, or 240 kHz for WFM). `nfm` is the audio low-pass of NFM.
fn config(
    demod: Demod,
    rate: f64,
    agc: bool,
    gain: f32,
    deemph_s: f64,
    pitch: f64,
    nfm: NfmLowPass,
) -> AnalogConfig {
    let mut c = AnalogConfig::new(mode_of(demod), rate);
    match demod {
        Demod::Nfm => c.audio = Some(AudioFilter::LowPass(nfm.filter())),
        Demod::Wfm => {
            c.decimation = 5;
            c.audio = Some(AudioFilter::LowPass(LowPass::Taps {
                taps: 121,
                cutoff_hz: 15_000.0,
            }));
        }
        _ => {}
    }
    if demod.is_fm() {
        c.dc_block_hz = Some(20.0);
        c.deemphasis_s = (deemph_s > 0.0).then_some(deemph_s);
    }
    (c.gain, c.am_normalise) = gain_of(demod, agc, gain);
    c.cw_pitch_hz = pitch.clamp(200.0, 1500.0);
    c
}

const DEMOD: Setting<field::Choice<Demod>> = Setting::choice("demod", "Demodulator", Demod::Nfm)
    .help("A change restarts the state. WFM needs a 240 kHz input.");
const AGC: Setting<field::Bool> = Setting::new("agc", "AGC", "", true);
const GAIN: Setting<field::F64> = Setting::new("gain", "Gain", "dB", 20.0).range(-20.0, 80.0, 0.0);
const DEEMPH: Setting<field::F64> =
    Setting::new("deemph", "De-emphasis", "µs", 0.0).range(0.0, 750.0, 0.0);
const PITCH: Setting<field::Hz> =
    Setting::new("pitch", "Pitch", "Hz", 700.0).range(200.0, 1500.0, 0.0);
const NFM_LOWPASS: Setting<field::Choice<NfmLowPass>> =
    Setting::choice("nfm_lowpass", "NFM audio low-pass", NfmLowPass::Khz3);

sdrkit_core::settings! {
    /// The settings of [`DemodComponent`].
    pub enum DemodSetting {
        /// The demodulator.
        Demod = DEMOD,
        /// The AGC.
        Agc = AGC,
        /// The gain when the AGC is off (dB).
        Gain = GAIN,
        /// The FM de-emphasis time constant (µs, 0 for none).
        Deemph = DEEMPH,
        /// The CW pitch (Hz).
        Pitch = PITCH,
        /// The audio low-pass of NFM.
        NfmLowpass = NFM_LOWPASS,
    }
}

/// The demodulator component (`general.demod`): the filtered channel in, 48 kHz `Real` audio and a [`DemodReport`] at each tick of
/// the graph clock out. Its command `restart` starts the state again.
pub struct DemodComponent {
    demod: Demod,
    agc: bool,
    gain_db: f64,
    deemph_us: f64,
    pitch: f64,
    nfm_lowpass: NfmLowPass,
    d: Option<AnalogDemod>,
    rate: f64,
}

impl Default for DemodComponent {
    fn default() -> Self {
        DemodComponent {
            demod: Demod::Nfm,
            agc: true,
            gain_db: 20.0,
            deemph_us: 0.0,
            pitch: 700.0,
            nfm_lowpass: NfmLowPass::Khz3,
            d: None,
            rate: 0.0,
        }
    }
}

impl DemodComponent {
    fn rebuild(&mut self) -> Result<(), ModemError> {
        if self.rate <= 0.0 {
            return Ok(());
        }
        // A WFM choice on a narrow input (or the reverse) waits for the reconfigure that follows.
        let demod = if (self.rate > 100e3) != self.demod.is_wide() {
            if self.rate > 100e3 {
                Demod::Wfm
            } else {
                Demod::Nfm
            }
        } else {
            self.demod
        };
        let c = config(
            demod,
            self.rate,
            self.agc,
            db_to_amp(self.gain_db),
            self.deemph_us * 1e-6,
            self.pitch,
            self.nfm_lowpass,
        );
        self.d = Some(AnalogDemod::new(c)?);
        Ok(())
    }
    /// Applies the AGC flag and the manual gain to the running demodulator.
    fn apply_gain(&mut self) {
        let (agc, gain) = (self.agc, db_to_amp(self.gain_db));
        let Some(d) = &mut self.d else { return };
        let (g, norm) = gain_of(demod_of(d.mode()), agc, gain);
        // The gain stage of `gain_of` is always one that the demodulator takes.
        let _ = d.set_gain(g);
        d.set_am_normalise(norm);
    }
}

impl Component for DemodComponent {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "general.demod",
            vec![Port::iq("in")],
            vec![
                Port::real("out").rate_from(RateFrom::Fixed(AUDIO_RATE)),
                Port::typed::<DemodReport>("report").rate_from(RateFrom::Fixed(0.0)),
            ],
        )
        .with_settings(DemodSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.rate = inputs[0].rate;
        self.rebuild()?;
        let bw = if self.rate > 100e3 { 15e3 } else { 6e3 };
        Ok(vec![
            Format {
                kind: Kind::Real,
                rate: AUDIO_RATE,
                bandwidth_hz: bw,
                ..inputs[0].clone()
            },
            Format::new(Kind::typed::<DemodReport>(), 0.0),
        ])
    }
    fn process(&mut self, io: &mut Io) {
        let x = io.iq(0);
        let Some(d) = self.d.as_mut() else { return };
        d.process(x, io.real_out(0), None);
        if io.tick() {
            let t = io.output(0).end() as f64 / AUDIO_RATE;
            let off = d.take_offset_hz();
            let fm = if d.mode().is_fm() { off } else { 0.0 };
            io.items_out::<DemodReport>(1).push(Timed {
                time: t,
                value: DemodReport { fm_offset_hz: fm },
            });
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        let refused = |e: ModemError| SettingError::Config {
            id,
            source: Box::new(ConfigError::from(e)),
        };
        match DemodSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            DemodSetting::Demod => {
                self.demod = DEMOD.read(id, v)?;
                self.rebuild().map_err(refused)?;
            }
            DemodSetting::Agc => {
                self.agc = AGC.read(id, v)?;
                self.apply_gain();
            }
            DemodSetting::Gain => {
                self.gain_db = GAIN.read(id, v)?;
                self.apply_gain();
            }
            DemodSetting::Deemph => {
                self.deemph_us = DEEMPH.read(id, v)?;
                if let Some(d) = &mut self.d {
                    if d.mode().is_fm() {
                        let tau = self.deemph_us * 1e-6;
                        d.set_deemphasis((tau > 0.0).then_some(tau))
                            .map_err(refused)?;
                    }
                }
            }
            DemodSetting::Pitch => {
                self.pitch = PITCH.read(id, v)?;
                if let Some(d) = &mut self.d {
                    d.set_pitch(self.pitch);
                }
            }
            DemodSetting::NfmLowpass => {
                let l = NFM_LOWPASS.read(id, v)?;
                if l != self.nfm_lowpass {
                    self.nfm_lowpass = l;
                    if self.d.as_ref().is_some_and(|d| d.mode() == AnalogMode::Nfm) {
                        self.rebuild().map_err(refused)?;
                    }
                }
            }
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([
            (DemodSetting::Demod, Value::Choice(self.demod.index())),
            (DemodSetting::Agc, Value::Bool(self.agc)),
            (DemodSetting::Gain, Value::F64(self.gain_db)),
            (DemodSetting::Deemph, Value::F64(self.deemph_us)),
            (DemodSetting::Pitch, Value::Hz(self.pitch)),
            (
                DemodSetting::NfmLowpass,
                Value::Choice(self.nfm_lowpass.index()),
            ),
        ])
    }
    fn commands(&self) -> &Commands {
        RestartCommand::commands()
    }
    fn invoke(&mut self, id: CommandId, _args: &Values) -> Result<Option<Record>, CommandError> {
        match RestartCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            RestartCommand::Restart => self.rebuild().map_err(|e| CommandError::Failed {
                source: StageError::new(e),
            })?,
        }
        Ok(None)
    }
}
