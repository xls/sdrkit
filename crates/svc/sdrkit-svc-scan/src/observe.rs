//! What a scanner sees of the channel it is tuned to, read from the results of any receiver, and the rule that turns it into
//! "active".

use sdrkit_core::publish::keys;
use sdrkit_core::record::FieldId;
use sdrkit_core::{LockState, Results, ResultsDecl, Value};
use std::sync::Arc;

/// The key of a stack's squelch state: yes if the squelch passes sound.
pub const SQUELCH_OPEN: &str = "squelch_open";
/// The key of the CTCSS or DCS tone that a stack heard, as text.
pub const TONE: &str = "tone";

/// One look at the tuned channel. Every part is optional: a stack publishes what it measures.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Observation {
    /// The squelch of the stack is open (it passes sound).
    pub squelch_open: Option<bool>,
    /// The power of the channel in the bandwidth of the stack, dBFS.
    pub level_dbfs: Option<f32>,
    /// The noise floor that the level is compared with, dBFS.
    pub noise_dbfs: Option<f32>,
    /// The signal to noise ratio, dB.
    pub snr_db: Option<f32>,
    /// The lock state of the stack: locked means that it decodes.
    pub lock_state: LockState,
    /// The CTCSS or DCS tone heard (`"CTCSS 88.5 Hz"`). Empty for none.
    pub tone: String,
}

impl Observation {
    /// The signal to noise ratio: the measured one, or the level minus the noise.
    pub fn snr(&self) -> Option<f32> {
        self.snr_db.or(match (self.level_dbfs, self.noise_dbfs) {
            (Some(l), Some(n)) => Some(l - n),
            _ => None,
        })
    }
}

/// Reads observations from the results of a receiver: the status lock state, SNR and level, the standard field
/// [`keys::NOISE_DBFS`], a yes or no field `SQUELCH_OPEN` and a text field `TONE`. It finds the field ids again only when
/// the results declaration changes.
#[derive(Clone, Debug, Default)]
pub struct Observer {
    decl: Option<Arc<ResultsDecl>>,
    noise: Option<FieldId>,
    squelch: Option<FieldId>,
    tone: Option<FieldId>,
}

impl Observer {
    /// Writes what `r` shows into `out`, in place.
    pub fn read(&mut self, r: &Results, out: &mut Observation) {
        if !self.decl.as_ref().is_some_and(|d| Arc::ptr_eq(d, r.decl())) {
            let d = r.decl();
            self.noise = d.field_id(keys::NOISE_DBFS.key);
            self.squelch = d.field_id(SQUELCH_OPEN);
            self.tone = d.field_id(TONE);
            self.decl = Some(Arc::clone(d));
        }
        let value = |id: Option<FieldId>| id.and_then(|id| r.fields().value(id));
        out.noise_dbfs = match value(self.noise) {
            Some(Value::F64(v)) => Some(*v as f32),
            _ => None,
        };
        out.squelch_open = match value(self.squelch) {
            Some(Value::Bool(b)) => Some(*b),
            _ => None,
        };
        match value(self.tone) {
            Some(Value::Text(t)) => {
                let t = t.trim();
                if out.tone != t {
                    out.tone.clear();
                    out.tone.push_str(t);
                }
            }
            _ => out.tone.clear(),
        }
        let s = r.status();
        out.level_dbfs = s.level_dbfs;
        out.lock_state = s.lock_state;
        out.snr_db = s.snr_db.or(match (s.level_dbfs, out.noise_dbfs) {
            (Some(l), Some(n)) => Some(l - n),
            _ => None,
        });
    }
}

sdrkit_core::choice_enum! {
    /// What makes a channel active, so that the scanner stops.
    pub enum StopOn {
        /// The squelch of the stack is open. If the stack has no squelch, the SNR is at least the threshold.
        Squelch = ("squelch", "Squelch open"),
        /// The SNR is at least the threshold.
        Snr = ("snr", "SNR"),
        /// The level is at least the threshold in dBFS.
        Level = ("level", "Level"),
        /// The stack is locked (it decodes).
        Lock = ("lock", "Lock"),
        /// The squelch is open or the stack is locked.
        SquelchOrLock = ("squelch_or_lock", "Squelch or lock"),
    }
}

/// The activity rule: what counts, and its threshold (dB of SNR, or dBFS for [`StopOn::Level`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Activity {
    /// What counts.
    pub stop_on: StopOn,
    /// The threshold.
    pub threshold_db: f32,
}

impl Default for Activity {
    fn default() -> Self {
        Activity {
            stop_on: StopOn::Squelch,
            threshold_db: 6.0,
        }
    }
}

impl Activity {
    /// True if the observation counts as activity.
    pub fn active(&self, o: &Observation) -> bool {
        let min = self.threshold_db;
        let snr_ok = || o.snr().is_some_and(|s| s >= min);
        match self.stop_on {
            StopOn::Squelch => o.squelch_open.unwrap_or_else(snr_ok),
            StopOn::Snr => snr_ok(),
            StopOn::Level => o.level_dbfs.is_some_and(|l| l >= min),
            StopOn::Lock => o.lock_state == LockState::Locked,
            StopOn::SquelchOrLock => {
                o.lock_state == LockState::Locked || o.squelch_open.unwrap_or_else(snr_ok)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sdrkit_core::publish::{field, Field, Published};
    use sdrkit_core::Status;

    const NOISE: Field<field::F64> = Field::new("noise_dbfs", "Noise", "dBFS");
    const SQUELCH: Field<field::Bool> = Field::new("squelch_open", "Squelch open", "");
    const TONE_F: Field<field::Text> = Field::new("tone", "Tone", "");
    const NOISE_TEXT: Field<field::Text> = Field::new("noise", "Noise", "");

    #[test]
    fn reads_the_typed_vocabulary() {
        let mut p = Published::new(&[&NOISE, &SQUELCH, &TONE_F, &NOISE_TEXT]);
        p.results_mut()
            .set_status(&Status::new(LockState::Locked).with_level_dbfs(-40.0));
        p.set(&NOISE, -70.0);
        p.set(&SQUELCH, true);
        p.set(&TONE_F, "CTCSS 88.5 Hz");
        let mut reader = Observer::default();
        let mut o = Observation::default();
        reader.read(p.results(), &mut o);
        assert_eq!(o.squelch_open, Some(true));
        assert_eq!(o.noise_dbfs, Some(-70.0));
        assert_eq!(o.snr(), Some(30.0));
        assert_eq!(o.tone, "CTCSS 88.5 Hz");
        p.set(&SQUELCH, false);
        reader.read(p.results(), &mut o);
        assert_eq!(o.squelch_open, Some(false));
        p.none(&SQUELCH);
        p.none(&NOISE);
        reader.read(p.results(), &mut o);
        assert_eq!((o.squelch_open, o.noise_dbfs), (None, None));
        // The text form of the noise is not read.
        p.set(&NOISE_TEXT, "-70.0 dBFS");
        reader.read(p.results(), &mut o);
        assert_eq!(o.noise_dbfs, None);
    }

    #[test]
    fn rules() {
        let quiet = Observation {
            snr_db: Some(2.0),
            ..Observation::default()
        };
        let loud = Observation {
            snr_db: Some(20.0),
            level_dbfs: Some(-30.0),
            ..Observation::default()
        };
        let sq = Activity::default();
        assert!(!sq.active(&quiet) && sq.active(&loud));
        let closed = Observation {
            squelch_open: Some(false),
            ..loud.clone()
        };
        assert!(!sq.active(&closed), "the squelch of the stack wins");
        let rule = |stop_on, threshold_db| Activity {
            stop_on,
            threshold_db,
        };
        assert!(rule(StopOn::Level, -35.0).active(&loud));
        assert!(!rule(StopOn::Lock, 0.0).active(&loud));
        let locked = Observation {
            lock_state: LockState::Locked,
            ..quiet.clone()
        };
        assert!(rule(StopOn::Lock, 0.0).active(&locked));
        assert!(rule(StopOn::SquelchOrLock, 6.0).active(&locked));
    }
}
