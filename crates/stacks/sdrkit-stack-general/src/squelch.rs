//! The level meter and the carrier squelch.

use crate::types::{NoiseEstimate, SquelchBlock, RESTART};
use sdrkit_algo_dsp::meter::power_to_db;
use sdrkit_algo_modem::squelch::Gate;
use sdrkit_core::publish::field;
use sdrkit_core::{
    Choice, Command, CommandError, CommandId, CommandSet, Commands, ConfigError, Record, Setting,
    SettingError, SettingId, SettingSet, Value, Values,
};
use sdrkit_graph::{
    values_of, Applied, Component, ComponentInfo, Format, Io, Kind, Port, RateFrom, Timed,
};

sdrkit_core::choice_enum! {
    /// The squelch kinds of the general receiver.
    #[derive(Default)]
    pub enum SquelchKind {
        /// Always open.
        Off = ("off", "Off"),
        /// Against the measured noise: opens 6 dB above it.
        #[default]
        Auto = ("auto", "Auto"),
        /// Against a level in dBFS.
        Level = ("level", "Level"),
    }
}

/// The number of blocks the squelch stays open after the last block that opened it (150 ms).
pub const HANG_BLOCKS: u32 = 15;

/// Auto opens 6 dB over the noise (4 times its power) and stays open down to 2.5 times its power.
const AUTO_DB: (f32, f32) = (6.020_6, 3.979_4);
/// Level stays open down to the threshold divided by 1.26 (1 dB of hysteresis).
const LEVEL_HYSTERESIS_DB: f32 = 1.003_7;

/// The carrier squelch as a plain object, one 10 ms block at a time.
#[derive(Clone, Debug)]
pub struct CarrierSquelch {
    /// The kind.
    pub kind: SquelchKind,
    /// The Level threshold (dBFS).
    pub level_db: f64,
    /// Use the in-passband noise (USB and LSB).
    pub ssb: bool,
    level: Option<f32>,
    noise: Option<NoiseEstimate>,
    gate: Gate,
    blocks_open: u64,
    blocks_closed: u64,
}

impl Default for CarrierSquelch {
    fn default() -> Self {
        CarrierSquelch {
            kind: SquelchKind::Auto,
            level_db: -60.0,
            ssb: false,
            level: None,
            noise: None,
            gate: new_gate(),
            blocks_open: 0,
            blocks_closed: 0,
        }
    }
}

/// The decision gate: it opens on one block, and it closes on the last of `HANG_BLOCKS` blocks that do not want it open.
fn new_gate() -> Gate {
    Gate::new(AUTO_DB.0, AUTO_DB.1, 1, HANG_BLOCKS - 1).expect("the thresholds are ordered")
}

impl CarrierSquelch {
    /// Restarts the level average, the noise estimate and the state (closed, no hang). The counters are kept.
    pub fn restart(&mut self) {
        self.level = None;
        self.noise = None;
        self.gate.reset();
    }
    /// Only the noise estimate restarts (a demodulator or width change).
    pub fn restart_noise(&mut self) {
        self.noise = None;
    }
    /// A new noise estimate.
    pub fn set_noise(&mut self, e: NoiseEstimate) {
        self.noise = Some(e);
    }
    /// The noise in the channel to compare with (linear power), when measured.
    pub fn noise(&self) -> Option<f32> {
        let e = self.noise?;
        if self.ssb {
            e.inside.or(e.beside)
        } else {
            e.beside
        }
    }
    /// One block of mean power `p`.
    pub fn block(&mut self, p: f32) -> SquelchBlock {
        const A: f32 = 0.153_518_7; // 1 - e^(-0.01/0.06)
        let level = match self.level {
            None => p,
            Some(l) => l + A * (p - l),
        };
        self.level = Some(level);
        let noise = self.noise();
        // The measure in dB and the gate's thresholds: SNR for Auto, dBFS for Level.
        let (measure, thresholds) = match self.kind {
            SquelchKind::Off => (Some(f32::INFINITY), AUTO_DB),
            SquelchKind::Auto => (
                match (self.noise, noise) {
                    (None, _) => None,
                    (Some(_), Some(n)) => {
                        Some(power_to_db(level as f64 / n.max(1e-30) as f64) as f32)
                    }
                    (Some(e), None) => e.impossible.then_some(f32::INFINITY),
                },
                AUTO_DB,
            ),
            SquelchKind::Level => {
                let t = self.level_db as f32;
                (
                    Some(power_to_db(level as f64) as f32),
                    (t, t - LEVEL_HYSTERESIS_DB),
                )
            }
        };
        let _ = self.gate.set_thresholds(thresholds.0, thresholds.1);
        let was_open = self.gate.is_open();
        let need = if was_open { thresholds.1 } else { thresholds.0 };
        let want = measure.is_some_and(|m| m >= need);
        let open = self.gate.update(measure);
        if open {
            self.blocks_open += 1;
        } else {
            self.blocks_closed += 1;
        }
        let threshold_db = match self.kind {
            SquelchKind::Off => -200.0,
            SquelchKind::Auto => noise.map_or(-200.0, |n| power_to_db(n as f64) as f32 + AUTO_DB.0),
            SquelchKind::Level => self.level_db as f32,
        };
        SquelchBlock {
            open,
            wanted: want,
            state: if open { 2 } else { 1 },
            level,
            noise,
            threshold_db,
            blocks_open: self.blocks_open,
            blocks_closed: self.blocks_closed,
        }
    }
}

/// The squelch component (`general.squelch`): the filtered channel (`in`) and the noise estimates (`noise`) in. Out: one
const KIND: Setting<field::Choice<SquelchKind>> =
    Setting::choice("kind", "Squelch", SquelchKind::Auto);
const LEVEL: Setting<field::F64> =
    Setting::new("level", "Squelch level", "dBFS", -60.0).range(-140.0, 20.0, 0.0);
const SSB: Setting<field::Bool> = Setting::new("ssb", "In-passband noise", "", false);

sdrkit_core::settings! {
    /// The settings of [`SquelchComponent`].
    pub enum SquelchSetting {
        /// The squelch kind.
        Kind = KIND,
        /// The threshold of the level squelch (dBFS).
        Level = LEVEL,
        /// The noise inside the passband is the noise (USB and LSB).
        Ssb = SSB,
    }
}

/// The command that drops the noise estimate.
const RESTART_NOISE: Command =
    Command::new("restart_noise", "Restart noise").help("Drops the noise estimate.");

sdrkit_core::commands! {
    /// The commands of [`SquelchComponent`].
    pub enum SquelchCommand {
        /// Starts the level average and the state again.
        Restart = RESTART,
        /// Drops the noise estimate.
        RestartNoise = RESTART_NOISE,
    }
}

/// The carrier squelch (`general.squelch`): a [`SquelchBlock`] per 10 ms on `out`, timed at the end of its block.
pub struct SquelchComponent {
    sq: CarrierSquelch,
    rate: f64,
    block: usize,
    acc: f64,
    fill: usize,
    pending: Vec<Timed<NoiseEstimate>>,
    /// Samples since the last configure (block times count from there).
    n: u64,
}

impl Default for SquelchComponent {
    fn default() -> Self {
        SquelchComponent {
            sq: CarrierSquelch::default(),
            rate: 0.0,
            block: 480,
            acc: 0.0,
            fill: 0,
            pending: Vec::new(),
            n: 0,
        }
    }
}

impl Component for SquelchComponent {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "general.squelch",
            vec![Port::iq("in"), Port::typed::<NoiseEstimate>("noise")],
            vec![Port::typed::<SquelchBlock>("out").rate_from(RateFrom::Fixed(0.0))],
        )
        .with_settings(SquelchSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.rate = inputs[0].rate;
        self.block = ((self.rate / 100.0).round() as usize).max(1);
        self.acc = 0.0;
        self.fill = 0;
        self.pending.clear();
        self.sq.restart();
        self.n = 0;
        Ok(vec![Format::new(Kind::typed::<SquelchBlock>(), 0.0)])
    }
    fn process(&mut self, io: &mut Io) {
        for e in io.items::<NoiseEstimate>(1) {
            if self.pending.len() >= 64 {
                self.pending.remove(0);
            }
            self.pending.push(e.clone());
        }
        let first = self.n;
        let x = io.iq(0);
        self.n += x.len() as u64;
        let mut i = 0;
        while i < x.len() {
            let take = (self.block - self.fill).min(x.len() - i);
            self.acc += x[i..i + take]
                .iter()
                .map(|v| v.norm_sqr() as f64)
                .sum::<f64>();
            self.fill += take;
            i += take;
            if self.fill == self.block {
                let t = (first + i as u64) as f64 / self.rate;
                while let Some(e) = self.pending.first() {
                    if e.time <= t + 1e-9 {
                        let e = self.pending.remove(0);
                        self.sq.set_noise(e.value);
                    } else {
                        break;
                    }
                }
                let p = (self.acc / self.block as f64) as f32;
                self.acc = 0.0;
                self.fill = 0;
                let b = self.sq.block(p);
                io.items_out::<SquelchBlock>(0)
                    .push(Timed { time: t, value: b });
            }
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match SquelchSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            SquelchSetting::Kind => self.sq.kind = KIND.read(id, v)?,
            SquelchSetting::Level => self.sq.level_db = LEVEL.read(id, v)?,
            SquelchSetting::Ssb => self.sq.ssb = SSB.read(id, v)?,
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([
            (SquelchSetting::Kind, Value::Choice(self.sq.kind.index())),
            (SquelchSetting::Level, Value::F64(self.sq.level_db)),
            (SquelchSetting::Ssb, Value::Bool(self.sq.ssb)),
        ])
    }
    fn commands(&self) -> &Commands {
        SquelchCommand::commands()
    }
    fn invoke(&mut self, id: CommandId, _args: &Values) -> Result<Option<Record>, CommandError> {
        match SquelchCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            SquelchCommand::Restart => {
                self.sq.restart();
                self.acc = 0.0;
                self.fill = 0;
                self.pending.clear();
            }
            SquelchCommand::RestartNoise => {
                self.sq.restart_noise();
                self.pending.clear();
            }
        }
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auto_opens_on_6_db_and_hangs() {
        let mut sq = CarrierSquelch::default();
        assert!(!sq.block(1.0).open, "closed before the first estimate");
        sq.set_noise(NoiseEstimate {
            beside: Some(0.1),
            inside: None,
            impossible: false,
        });
        sq.restart();
        sq.set_noise(NoiseEstimate {
            beside: Some(0.1),
            inside: None,
            impossible: false,
        });
        assert!(!sq.block(0.3).open);
        assert!(sq.block(10.0).open);
        sq.restart();
        sq.set_noise(NoiseEstimate {
            beside: Some(0.1),
            inside: None,
            impossible: false,
        });
        sq.block(1.0);
        let mut n = 0;
        while sq.block(0.0).open {
            n += 1;
        }
        // The 60 ms level average falls below 2.5 x noise after about 8 blocks, then the 15-block hang.
        assert!((20..=24).contains(&n), "{n}");
        let mut s = CarrierSquelch::default();
        s.set_noise(NoiseEstimate {
            beside: None,
            inside: None,
            impossible: true,
        });
        assert!(s.block(0.0).open, "no noise measurable: always open");
    }
}
