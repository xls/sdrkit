//! The report of the general receiver: the state, the values published every 0.1 s and the summary line. [`Report`]
//! (`general.report`) collects what the other components measured and writes the typed results.

use crate::amproc::AmReport;
use crate::toneid::ToneId;
use crate::typed::{GeneralFields, WidthPresetRow, PRESETS as PRESET_TABLE, WIDTH_PRESETS};
use crate::types::{
    BlankerReport, Demod, DemodReport, RestartCommand, SpeechProbability, SquelchBlock,
};
use sdrkit_algo_dsp::meter::power_to_db;
use sdrkit_core::interface::{Fields, Hertz};
use sdrkit_core::publish::field;
use sdrkit_core::{
    Choice, CommandError, CommandId, CommandSet, Commands, ConfigError, LockState, Published,
    Record, Results, Setting, SettingError, SettingId, SettingSet, Status, Value, Values,
};
use sdrkit_graph::{values_of, Applied, Component, ComponentInfo, Format, Io, Kept, Port};
use std::fmt::Write;

/// A width in kHz with up to 4 significant digits ("12.5", "0.5", "180"), written to `out`.
pub fn write_khz(out: &mut String, w: f64) {
    let k = w / 1000.0;
    let digits = if k >= 100.0 {
        1
    } else if k >= 10.0 {
        2
    } else if k >= 1.0 {
        3
    } else {
        4
    };
    let start = out.len();
    let _ = write!(out, "{:.*}", digits, k);
    if out[start..].contains('.') {
        while out.ends_with('0') {
            out.pop();
        }
        if out.ends_with('.') {
            out.pop();
        }
    }
}

/// What the summary line says.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Summary {
    /// The demodulator.
    pub demod: Demod,
    /// The filter width (Hz).
    pub width_hz: f64,
    /// The level (dBFS), or `None` before the first block.
    pub level_dbfs: Option<f64>,
    /// The SNR (dB), when the noise is measured.
    pub snr_db: Option<f64>,
    /// The squelch is open.
    pub squelch_open: bool,
}

/// Writes the summary line for logs and the status bar to `out` ("NFM 12.5 kHz, -10.0 dBFS, SNR 30.2 dB, squelch open").
pub fn write_summary(out: &mut String, s: &Summary) {
    out.push_str(s.demod.label());
    out.push(' ');
    write_khz(out, s.width_hz);
    out.push_str(" kHz");
    let Some(level) = s.level_dbfs else {
        out.push_str(", no signal yet");
        return;
    };
    let _ = write!(out, ", {level:.1} dBFS");
    if let Some(snr) = s.snr_db {
        let _ = write!(out, ", SNR {snr:.1} dB");
    }
    out.push_str(if s.squelch_open {
        ", squelch open"
    } else {
        ", squelch closed"
    });
}

const DEMOD: Setting<field::Choice<Demod>> = Setting::choice("demod", "Demodulator", Demod::Nfm);
const WIDTH: Setting<field::Hz> =
    Setting::new("width", "Width", "Hz", 12_500.0).range(0.0, 1e6, 0.0);
const CHANNEL_OFFSET: Setting<field::Hz> =
    Setting::new("channel_offset", "Channel offset", "Hz", 0.0).range(-1e9, 1e9, 0.0);
const DIAL_HZ: Setting<field::Hz> =
    Setting::new("dial_hz", "Dial", "Hz", 0.0).range(0.0, 1e12, 0.0);
const INPUT_RATE: Setting<field::Hz> =
    Setting::new("input_rate", "Input rate", "Hz", 0.0).range(0.0, 1e9, 0.0);
const PRESETS: Setting<field::Text> = Setting::new("presets", "Presets", "", "");

sdrkit_core::settings! {
    /// The settings of [`Report`]: values the receiver holds and the components do not measure.
    pub enum ReportSetting {
        /// The demodulator.
        Demod = DEMOD,
        /// The filter width (Hz).
        Width = WIDTH,
        /// The channel relative to the dial (Hz).
        ChannelOffset = CHANNEL_OFFSET,
        /// The dial frequency (Hz).
        DialHz = DIAL_HZ,
        /// The input rate (Hz).
        InputRate = INPUT_RATE,
        /// The presets, in the form of the stack setting `presets`.
        Presets = PRESETS,
    }
}

/// The report component (`general.report`): a sink of the squelch decisions, the audio, the reports of the demodulator, the AM
/// processor, the blanker and the voice noise remover, and the tone identification. It writes the results at each tick of the
/// graph clock. Its command `restart` shows no signal until the next squelch block.
pub struct Report {
    demod: Demod,
    width_hz: f64,
    channel_offset_hz: f64,
    dial_hz: f64,
    input_rate_hz: f64,
    presets: String,
    last_sq: Option<SquelchBlock>,
    audio_sum: f64,
    audio_n: u64,
    fm_offset: f64,
    am: AmReport,
    nb_pct: f32,
    speech: f32,
    tone: ToneId,
    text: String,
    dcs: String,
    alias: String,
    label: String,
    p: Published,
    widths_of: Option<Demod>,
}

impl Default for Report {
    fn default() -> Self {
        let mut p = Published::new(GeneralFields::SHAPE.fields)
            .with_tables(&[WIDTH_PRESETS.def(), PRESET_TABLE.def()]);
        p.results_mut()
            .set_status(&Status::new(LockState::Searching));
        Report {
            demod: Demod::Nfm,
            width_hz: 12_500.0,
            channel_offset_hz: 0.0,
            dial_hz: 0.0,
            input_rate_hz: 0.0,
            presets: String::new(),
            last_sq: None,
            audio_sum: 0.0,
            audio_n: 0,
            fm_offset: 0.0,
            am: AmReport::default(),
            nb_pct: 0.0,
            speech: 0.0,
            tone: ToneId::default(),
            text: String::with_capacity(96),
            dcs: String::with_capacity(16),
            alias: String::with_capacity(16),
            label: String::with_capacity(32),
            p,
            widths_of: None,
        }
    }
}

impl Report {
    fn publish(&mut self) {
        let demod = self.demod;
        let p = &mut self.p;
        if self.widths_of != Some(demod) {
            self.widths_of = Some(demod);
            p.clear_rows(&WIDTH_PRESETS);
            for (i, hz) in demod.presets().iter().enumerate() {
                self.text.clear();
                let _ = write!(self.text, "{i:02}");
                WIDTH_PRESETS.put(
                    p,
                    &self.text,
                    &WidthPresetRow {
                        width_hz: Hertz(*hz),
                    },
                );
            }
        }
        let (lo, hi) = demod.edges(self.width_hz);
        let listening = self.dial_hz + self.channel_offset_hz;
        let sq = self.last_sq;
        let level = sq.map(|b| power_to_db(f64::from(b.level)).max(-200.0));
        let noise = sq
            .and_then(|b| b.noise)
            .map(|n| power_to_db(f64::from(n)).max(-200.0));
        let snr = level.zip(noise).map(|(l, n)| l - n);
        let open = sq.is_some_and(|b| b.open);
        let offset = (demod.is_fm() && open).then_some(self.fm_offset);
        let audio = (self.audio_n > 0)
            .then(|| power_to_db(self.audio_sum / self.audio_n as f64))
            .filter(|db| *db > -199.0);
        self.audio_sum = 0.0;
        self.audio_n = 0;
        let blanked = if self.am.active {
            self.am.blanked_pct
        } else {
            self.nb_pct
        };
        let t = &self.tone;
        self.dcs.clear();
        self.alias.clear();
        self.label.clear();
        if let Some(r) = t.dcs {
            let _ = write!(self.dcs, "{}{}", r.code, polarity(r.inverted));
        }
        if let Some((c, inv)) = t.dcs.and_then(|r| r.alias) {
            let _ = write!(self.alias, "{c}{}", polarity(inv));
        }
        match (t.ctcss_hz, t.dcs) {
            (Some(hz), _) => {
                let _ = write!(self.label, "CTCSS {hz:.1} Hz");
            }
            (None, Some(r)) => {
                let _ = write!(self.label, "DCS {}{}", r.code, polarity(r.inverted));
            }
            _ => {}
        }
        p.put(&GeneralFields {
            demod,
            width_hz: Hertz(self.width_hz),
            input_rate_hz: Hertz(self.input_rate_hz),
            demod_rate_hz: Hertz(demod.rate()),
            channel_offset_hz: Hertz(self.channel_offset_hz),
            dial_hz: Hertz(self.dial_hz),
            listening_hz: Hertz(listening),
            filter_lo_hz: Hertz(lo),
            filter_hi_hz: Hertz(hi),
            passband_lo_hz: Hertz(listening + lo),
            passband_hi_hz: Hertz(listening + hi),
            level_dbfs: level,
            noise_dbfs: noise,
            snr_db: snr,
            squelch_threshold_dbfs: sq
                .map(|b| f64::from(b.threshold_db))
                .filter(|t| *t > -199.0),
            squelch_open: open,
            offset_hz: offset.map(Hertz),
            audio_dbfs: audio,
            blocks_open: sq.map(|b| b.blocks_open as i64),
            blocks_closed: sq.map(|b| b.blocks_closed as i64),
            blanked_pct: f64::from(blanked),
            speech_probability: f64::from(self.speech),
            needle: level.map(crate::bands::needle_position),
            ctcss_hz: t.ctcss_hz.map(Hertz),
            ctcss_measured_hz: t.ctcss_hz.map(|_| Hertz(t.ctcss_exact_hz)),
            ctcss_db: t.ctcss_hz.map(|_| f64::from(t.ctcss_db)),
            dcs: text(&self.dcs),
            dcs_alias: text(&self.alias),
            tone: text(&self.label),
            tone_gate_open: t.gate_open,
        });
        self.text.clear();
        write_summary(
            &mut self.text,
            &Summary {
                demod,
                width_hz: self.width_hz,
                level_dbfs: level,
                snr_db: snr,
                squelch_open: open,
            },
        );
        let lock_state = match sq {
            None => LockState::Searching,
            Some(b) if b.state < 2 => LockState::Tracking,
            Some(_) => LockState::Locked,
        };
        let text = &self.text;
        self.p.results_mut().update_status(|s| {
            let next = (
                lock_state,
                snr.map(|v| v as f32),
                level.map(|v| v as f32),
                Some(offset.unwrap_or(0.0)),
            );
            if (s.lock_state, s.snr_db, s.level_dbfs, s.offset_hz) == next && s.summary == *text {
                return false;
            }
            (s.lock_state, s.snr_db, s.level_dbfs, s.offset_hz) = next;
            s.summary.clone_from(text);
            true
        });
    }
}

/// `s`, or no value if it is empty.
fn text(s: &str) -> Option<&str> {
    (!s.is_empty()).then_some(s)
}

/// The polarity letter of a DCS code: `I` for inverted, `N` for normal.
fn polarity(inverted: bool) -> char {
    if inverted {
        'I'
    } else {
        'N'
    }
}

impl Component for Report {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::sink(
            "general.report",
            vec![
                Port::typed::<SquelchBlock>("sq"),
                Port::real("audio"),
                Port::typed::<DemodReport>("demod"),
                Port::typed::<AmReport>("am"),
                Port::typed::<BlankerReport>("nb"),
                Port::typed::<SpeechProbability>("speech"),
                Port::typed::<ToneId>("tone"),
            ],
        )
        .with_settings(ReportSetting::schema())
    }
    fn configure(&mut self, _inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.last_sq = None;
        self.audio_sum = 0.0;
        self.audio_n = 0;
        Ok(vec![])
    }
    fn process(&mut self, io: &mut Io) {
        if let Some(b) = io.items::<SquelchBlock>(0).last() {
            self.last_sq = Some(b.value);
        }
        for &v in io.real(1) {
            self.audio_sum += f64::from(v * v);
        }
        self.audio_n += io.real(1).len() as u64;
        if let Some(r) = io.items::<DemodReport>(2).last() {
            self.fm_offset = r.value.fm_offset_hz;
        }
        if let Some(a) = io.items::<AmReport>(3).last() {
            self.am = a.value;
        }
        if let Some(b) = io.items::<BlankerReport>(4).last() {
            self.nb_pct = b.value.blanked_pct;
        }
        if let Some(s) = io.items::<SpeechProbability>(5).last() {
            self.speech = s.value.0;
        }
        if let Some(t) = io.items::<ToneId>(6).last() {
            self.tone = t.value;
        }
        if io.tick() {
            self.publish();
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match ReportSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            ReportSetting::Demod => {
                self.demod = DEMOD.read(id, v)?;
                if self.demod != Demod::Am {
                    self.am = AmReport::default();
                }
                self.speech = 0.0;
            }
            ReportSetting::Width => self.width_hz = WIDTH.read(id, v)?,
            ReportSetting::ChannelOffset => self.channel_offset_hz = CHANNEL_OFFSET.read(id, v)?,
            ReportSetting::DialHz => self.dial_hz = DIAL_HZ.read(id, v)?,
            ReportSetting::InputRate => self.input_rate_hz = INPUT_RATE.read(id, v)?,
            ReportSetting::Presets => {
                let t = PRESETS.read(id, v)?;
                let list =
                    crate::presets::parse(t).ok_or(SettingError::Malformed { id, at: None })?;
                self.p.clear_rows(&PRESET_TABLE);
                for preset in &list {
                    preset.write_row(&mut self.p);
                }
                t.clone_into(&mut self.presets);
            }
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([
            (ReportSetting::Demod, Value::Choice(self.demod.index())),
            (ReportSetting::Width, Value::Hz(self.width_hz)),
            (
                ReportSetting::ChannelOffset,
                Value::Hz(self.channel_offset_hz),
            ),
            (ReportSetting::DialHz, Value::Hz(self.dial_hz)),
            (ReportSetting::InputRate, Value::Hz(self.input_rate_hz)),
            (ReportSetting::Presets, Value::Text(self.presets.clone())),
        ])
    }
    fn results(&self) -> Option<&Results> {
        Some(self.p.results())
    }
    /// Forgets the measurements of the frequency before and writes the results again. The presets and the width
    /// presets stay if the stack keeps them.
    fn forget(&mut self, kept: &Kept) {
        self.last_sq = None;
        self.audio_sum = 0.0;
        self.audio_n = 0;
        self.fm_offset = 0.0;
        self.am = AmReport::default();
        self.nb_pct = 0.0;
        self.speech = 0.0;
        self.tone = ToneId::default();
        if !kept.table(PRESET_TABLE.key) {
            self.p.clear_rows(&PRESET_TABLE);
        }
        if !kept.table(WIDTH_PRESETS.key) {
            self.p.clear_rows(&WIDTH_PRESETS);
        }
        self.publish();
    }
    fn commands(&self) -> &Commands {
        RestartCommand::commands()
    }
    fn invoke(&mut self, id: CommandId, _args: &Values) -> Result<Option<Record>, CommandError> {
        match RestartCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            RestartCommand::Restart => self.last_sq = None,
        }
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn khz(w: f64) -> String {
        let mut s = String::new();
        write_khz(&mut s, w);
        s
    }

    #[test]
    fn widths_and_summaries() {
        assert_eq!(khz(12_500.0), "12.5");
        assert_eq!(khz(500.0), "0.5");
        assert_eq!(khz(180_000.0), "180");
        assert_eq!(khz(6250.0), "6.25");
        let mut s = Summary {
            demod: Demod::Nfm,
            width_hz: 12_500.0,
            level_dbfs: None,
            snr_db: None,
            squelch_open: false,
        };
        let mut line = String::new();
        write_summary(&mut line, &s);
        assert_eq!(line, "NFM 12.5 kHz, no signal yet");
        s.level_dbfs = Some(-10.04);
        s.snr_db = Some(30.2);
        s.squelch_open = true;
        line.clear();
        write_summary(&mut line, &s);
        assert_eq!(line, "NFM 12.5 kHz, -10.0 dBFS, SNR 30.2 dB, squelch open");
    }
}
