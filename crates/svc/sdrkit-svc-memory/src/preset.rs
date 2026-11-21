//! Presets of a stack: a named set of its settings, and a frequency, rate, bandwidth, gains, squelch and audio if given.
//! One preset of each stack can be the default. The built-in presets cannot be renamed or deleted.

use crate::error::MemoryError;
use sdrkit_core::{OwnerKey, OwnerSetting, Schema, Value, Values};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeMap;
use std::sync::Arc;

/// A preset.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Preset {
    /// Unique in its store.
    pub id: u64,
    /// The stack id it belongs to.
    pub stack: String,
    /// Its name, unique within the stack (ignoring case).
    pub name: String,
    /// Starred presets list first.
    pub starred: bool,
    /// Shipped with the framework; cannot be deleted or renamed.
    pub builtin: bool,
    /// Where to tune, Hz; `None` keeps the frequency.
    pub freq_hz: Option<f64>,
    /// The sample rate, Hz; `None` keeps it.
    pub rate_hz: Option<f64>,
    /// The channel bandwidth, Hz; `None` keeps it.
    pub bandwidth_hz: Option<f64>,
    /// The submode of the stack (the demodulator of the general receiver). Empty for its default.
    pub submode: String,
    /// The gains of the radio, each a setting of the radio's owner.
    pub gains: Vec<OwnerSetting>,
    /// The squelch in the settings JSON form of the stack's `squelch` setting (`"auto"`, `"-60"`). Empty keeps it.
    pub squelch: String,
    /// Audio settings, each with its owner, applied by the host.
    pub audio: Vec<OwnerSetting>,
    /// The settings of the stack, each with its owner.
    pub settings: Vec<OwnerSetting>,
}

impl Preset {
    /// An empty preset of `stack` named `name`.
    pub fn new(stack: &str, name: &str) -> Preset {
        Preset {
            stack: stack.into(),
            name: name.into(),
            ..Preset::default()
        }
    }

    /// The same preset with setting `key` of `owner` set to `value`.
    pub fn with_setting(mut self, owner: OwnerKey, key: &str, value: Value) -> Preset {
        crate::memory::put_setting(&mut self.settings, owner, key, value);
        self
    }

    /// The values of `schema`, the schema of owner `owner`, with the settings of this preset for that owner over the defaults.
    /// A setting that the schema does not accept any more stays at its default. The squelch goes to the setting `squelch` if
    /// the schema has one.
    pub fn resolve(&self, owner: &OwnerKey, schema: &Arc<Schema>) -> Values {
        let mut values = Values::defaults(schema);
        for s in self.settings.iter().filter(|s| s.owner == *owner) {
            if let Ok(id) = schema.id(&s.key) {
                // A refused value leaves the default: the settings of a stack can change after the preset was saved.
                let _ = values.set(id, &s.value);
            }
        }
        if !self.squelch.is_empty() {
            let value = serde_json::from_str(&self.squelch).unwrap_or_else(|_| json!(self.squelch));
            let _ = values.update_from_json(&json!({ "squelch": value }));
        }
        values
    }
}

/// The presets of every stack, with the default per stack.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PresetStore {
    /// The user's presets (the built-in ones are added at load, not saved).
    pub presets: Vec<Preset>,
    /// The default preset id of each stack.
    pub defaults: BTreeMap<String, u64>,
    next_id: u64,
}

impl Default for PresetStore {
    fn default() -> Self {
        PresetStore::with_builtins()
    }
}

/// The most presets a store keeps.
pub const MAX_PRESETS: usize = 1000;

/// Ids at and above this are built-in presets.
const BUILTIN_BASE: u64 = 1 << 40;

impl PresetStore {
    /// A store with no presets at all.
    pub fn empty() -> PresetStore {
        PresetStore {
            presets: Vec::new(),
            defaults: BTreeMap::new(),
            next_id: 1,
        }
    }

    /// A store with the built-in presets ([`builtin_presets`]).
    pub fn with_builtins() -> PresetStore {
        let mut s = PresetStore::empty();
        s.add_builtins();
        s
    }

    /// Adds the built-in presets that are missing (after loading a file, which holds only the user's).
    pub fn add_builtins(&mut self) {
        for (i, mut p) in builtin_presets().into_iter().enumerate() {
            p.id = BUILTIN_BASE + i as u64;
            p.builtin = true;
            if !self.presets.iter().any(|q| q.id == p.id) {
                self.presets.push(p);
            }
        }
    }

    /// The user's presets only, for saving.
    pub fn user_presets(&self) -> impl Iterator<Item = &Preset> {
        self.presets.iter().filter(|p| !p.builtin)
    }

    /// A copy without the built-ins, the form to save.
    pub fn to_saved(&self) -> PresetStore {
        PresetStore {
            presets: self.user_presets().cloned().collect(),
            defaults: self.defaults.clone(),
            next_id: self.next_id,
        }
    }

    /// The preset with an id.
    pub fn get(&self, id: u64) -> Option<&Preset> {
        self.presets.iter().find(|p| p.id == id)
    }

    fn name_taken(&self, stack: &str, name: &str, except: u64) -> bool {
        self.presets
            .iter()
            .any(|p| p.id != except && p.stack == stack && p.name.eq_ignore_ascii_case(name))
    }

    /// Saves a preset. A user preset of the same stack and name is replaced (keeping its id and star); otherwise it is added. Returns
    /// its id.
    pub fn save(&mut self, mut p: Preset) -> Result<u64, MemoryError> {
        let name = p.name.trim().to_string();
        if name.is_empty() {
            return Err(MemoryError::PresetName);
        }
        p.name = name;
        p.builtin = false;
        if let Some(e) = self
            .presets
            .iter_mut()
            .find(|q| !q.builtin && q.stack == p.stack && q.name.eq_ignore_ascii_case(&p.name))
        {
            p.id = e.id;
            p.starred |= e.starred;
            *e = p;
            return Ok(e.id);
        }
        if self.name_taken(&p.stack, &p.name, 0) {
            return Err(MemoryError::PresetName);
        }
        if self.presets.len() >= MAX_PRESETS {
            return Err(MemoryError::Full { cap: MAX_PRESETS });
        }
        self.next_id = self
            .next_id
            .max(self.user_presets().map(|p| p.id).max().unwrap_or(0) + 1);
        p.id = self.next_id;
        self.next_id += 1;
        let id = p.id;
        self.presets.push(p);
        Ok(id)
    }

    /// Renames a user preset.
    pub fn rename(&mut self, id: u64, name: &str) -> Result<(), MemoryError> {
        let p = self.get(id).ok_or(MemoryError::NoItem { id })?;
        if p.builtin {
            return Err(MemoryError::Builtin);
        }
        let name = name.trim();
        if name.is_empty() || self.name_taken(&p.stack.clone(), name, id) {
            return Err(MemoryError::PresetName);
        }
        if let Some(p) = self.presets.iter_mut().find(|p| p.id == id) {
            p.name = name.into();
        }
        Ok(())
    }

    /// Stars or unstars a preset (built-in ones too).
    pub fn star(&mut self, id: u64, starred: bool) -> Result<(), MemoryError> {
        let p = self
            .presets
            .iter_mut()
            .find(|p| p.id == id)
            .ok_or(MemoryError::NoItem { id })?;
        p.starred = starred;
        Ok(())
    }

    /// Deletes a user preset (and its default mark).
    pub fn delete(&mut self, id: u64) -> Result<(), MemoryError> {
        let p = self.get(id).ok_or(MemoryError::NoItem { id })?;
        if p.builtin {
            return Err(MemoryError::Builtin);
        }
        self.presets.retain(|p| p.id != id);
        self.defaults.retain(|_, d| *d != id);
        Ok(())
    }

    /// Makes a preset the default of its stack, or (with `None`) clears the default of `stack`.
    pub fn set_default(&mut self, stack: &str, id: Option<u64>) -> Result<(), MemoryError> {
        match id {
            Some(id) => {
                let p = self.get(id).ok_or(MemoryError::NoItem { id })?;
                if p.stack != stack {
                    return Err(MemoryError::OtherStack);
                }
                self.defaults.insert(stack.into(), id);
            }
            None => {
                self.defaults.remove(stack);
            }
        }
        Ok(())
    }

    /// The preset to apply when `stack` starts, if one is chosen.
    pub fn default_for(&self, stack: &str) -> Option<&Preset> {
        self.defaults.get(stack).and_then(|id| self.get(*id))
    }
}

/// The built-in presets: common uses of the general receiver, with channels from public allocations.
pub(crate) fn builtin_presets() -> Vec<Preset> {
    let g = |name: &str, hz: f64, demod: &str, width: f64| {
        let mut p = Preset::new("general", name).with_setting(
            OwnerKey::of("stack.general"),
            "width",
            Value::F64(width),
        );
        p.submode = demod.into();
        p.freq_hz = Some(hz);
        p.squelch = "auto".into();
        p
    };
    vec![
        g("Airband 25 kHz (AM)", 121.5e6, "am", 8000.0),
        g("Airband 8.33 kHz (AM)", 118.0e6, "am", 5000.0),
        g("Marine VHF channel 16", 156.8e6, "nfm", 12500.0),
        g("PMR446", 446.006_25e6, "nfm", 8500.0),
        g("Amateur 2 m FM calling", 145.5e6, "nfm", 12500.0),
        g("Amateur 70 cm FM calling", 433.5e6, "nfm", 12500.0),
        g("CB channel 9 (AM)", 27.065e6, "am", 8000.0),
        g("Shortwave broadcast (AM)", 9.4e6, "am", 9000.0),
        g("Amateur 40 m (LSB)", 7.1e6, "lsb", 2700.0),
        g("Amateur 20 m (USB)", 14.2e6, "usb", 2700.0),
        g("CW", 7.03e6, "cw", 500.0),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use sdrkit_core::publish::field;
    use sdrkit_core::Setting;

    sdrkit_core::choice_enum! {
        enum Demod { Nfm = ("nfm", "NFM"), Am = ("am", "AM") }
    }
    sdrkit_core::choice_enum! {
        enum Squelch { Off = ("off", "Off"), Auto = ("auto", "Auto") }
    }
    const DEMOD: Setting<field::Choice<Demod>> =
        Setting::choice("demod", "Demodulator", Demod::Nfm);
    const WIDTH: Setting<field::F64> =
        Setting::new("width", "Filter", "Hz", 0.0).range(0.0, 220_000.0, 10.0);
    const SQUELCH: Setting<field::Choice<Squelch>> =
        Setting::choice("squelch", "Squelch", Squelch::Off);

    fn schema() -> Arc<Schema> {
        Arc::new(Schema::of(&[&DEMOD, &WIDTH, &SQUELCH]))
    }

    #[test]
    fn save_replace_rename_star_delete() {
        let mut s = PresetStore::empty();
        let a = s.save(Preset::new("general", "Local net")).unwrap();
        s.star(a, true).unwrap();
        let again = s.save(Preset::new("general", "local NET")).unwrap();
        assert_eq!(a, again, "same name replaces");
        assert!(s.get(a).unwrap().starred, "and keeps the star");
        let b = s.save(Preset::new("general", "Other")).unwrap();
        assert_eq!(s.rename(b, "Local net"), Err(MemoryError::PresetName));
        s.rename(b, "Renamed").unwrap();
        assert!(s.get(a).unwrap().starred);
        s.set_default("general", Some(b)).unwrap();
        assert_eq!(s.default_for("general").unwrap().id, b);
        s.delete(b).unwrap();
        assert!(s.default_for("general").is_none());
        assert_eq!(
            s.save(Preset::new("fm", "  ")),
            Err(MemoryError::PresetName)
        );
    }

    #[test]
    fn builtins_are_kept_out_of_saves_and_protected() {
        let mut s = PresetStore::with_builtins();
        let b = s.presets.iter().find(|p| p.builtin).unwrap().id;
        assert_eq!(s.delete(b), Err(MemoryError::Builtin));
        assert_eq!(s.rename(b, "x"), Err(MemoryError::Builtin));
        s.star(b, true).unwrap();
        let u = s.save(Preset::new("general", "Mine")).unwrap();
        let saved = s.to_saved();
        assert_eq!(saved.presets.len(), 1);
        let json = serde_json::to_string(&saved).unwrap();
        let mut loaded: PresetStore = serde_json::from_str(&json).unwrap();
        loaded.add_builtins();
        assert_eq!(loaded.presets.len(), s.presets.len());
        let next = loaded.save(Preset::new("general", "Next")).unwrap();
        assert!(next > u && next < BUILTIN_BASE);
        // A user preset may not take a built-in's name.
        let name = builtin_presets()[0].name.clone();
        assert!(loaded.save(Preset::new("general", &name)).is_err());
    }

    #[test]
    fn resolve_and_reset() {
        let general = OwnerKey::of("stack.general");
        let mut p = Preset::new("general", "Air")
            .with_setting(general.clone(), "demod", Value::Choice(1))
            .with_setting(general.clone(), "width", Value::F64(8000.0))
            .with_setting(general.clone(), "gone", Value::I64(1))
            .with_setting(OwnerKey::of("radio"), "width", Value::F64(1.0));
        p.squelch = "auto".into();
        let v = p.resolve(&general, &schema());
        assert_eq!(v.get(&DEMOD), Demod::Am);
        assert_eq!(v.get(&WIDTH), 8000.0);
        assert_eq!(v.get(&SQUELCH), Squelch::Auto);
        assert!(v.schema().id("gone").is_err());
        let d = Values::defaults(&schema());
        assert_eq!(d.get(&DEMOD), Demod::Nfm);
        assert_eq!(d.get(&SQUELCH), Squelch::Off);
    }
}
