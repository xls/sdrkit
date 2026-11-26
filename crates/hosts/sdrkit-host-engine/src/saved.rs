//! Settings and state kept by the host: the settings of every owner as one saved settings blob ([`crate::settings_blob`]),
//! the JSON Schema of an owner's settings, and the saved state of the engine (its radios, sessions and services) as one
//! file of bytes.

use crate::engine_owner::EngineOwner;
use crate::error::{ImportError, RestoreItem};
use crate::feed::lock;
use crate::services::ServiceKind;
use crate::session::{By, Session};
use crate::settings_blob::{self, BlobError, Stored};
use crate::{EngineError, Input, OwnerRef, RadioId, SessionId, Shared, TRANSMIT_ALLOWED, TUNED};
use sdrkit_core::settings::ValuesError;
use sdrkit_core::{owner, Owner, Schema, SettingId, StateBlob, Value, ValueKind, Values};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value as Json};
use std::collections::BTreeMap;
use std::sync::Arc;

/// The version of the state file.
const VERSION: u64 = 1;

/// The settings that a settings blob never holds, by owner key: every run starts with transmit off.
const NOT_SAVED: &[(&str, &str)] = &[("engine", TRANSMIT_ALLOWED.key())];

fn not_saved(owner: &str, setting: &str) -> bool {
    NOT_SAVED.iter().any(|(o, k)| *o == owner && *k == setting)
}

/// The settings of every owner that has values, as a settings blob. Fixed settings, settings without a value and the
/// settings of [`NOT_SAVED`] are left out.
pub(crate) fn export(sh: &Shared) -> Result<Vec<u8>, EngineError> {
    let mut owners = Vec::new();
    for (key, r) in sh.owners_with_values() {
        let entries = sh.owner(r, |o| o.values().map(|v| entries_of(&key, v)))?;
        if let Some(mut entries) = entries {
            // A rate that the engine set for a stack is saved as Automatic: it is not a rate of the host.
            let automatic = match r {
                OwnerRef::Radio(id) => sh.radio(id).is_some_and(|x| x.rate_is_automatic()),
                _ => false,
            };
            if automatic {
                let rate = sdrkit_driver_core::RATE.field.key;
                for (_, stored) in entries.iter_mut().filter(|(k, _)| k == rate) {
                    *stored = Stored::Value(Value::Hz(0.0));
                }
            }
            owners.push(settings_blob::Owner { key, entries });
        }
    }
    settings_blob::encode(&owners).ok_or(EngineError::Encode)
}

/// The saved entries of the values `v` of owner `owner`.
fn entries_of(owner: &str, v: &Values) -> Vec<(String, Stored)> {
    let mut out = Vec::new();
    for (i, s) in v.schema().settings().iter().enumerate() {
        let key = s.field.key.as_ref();
        if s.fixed || not_saved(owner, key) {
            continue;
        }
        let stored = match v.value(SettingId(i as u16)) {
            None | Some(Value::None) => continue,
            Some(Value::Choice(c)) => match s.field.choices.get(usize::from(*c)) {
                Some(c) => Stored::Choice(c.key.to_string()),
                None => continue,
            },
            Some(x) => Stored::Value(x.clone()),
        };
        out.push((key.to_owned(), stored));
    }
    out
}

/// Reads a settings blob and applies it to the owners it names. An owner that the engine does not have and a setting that
/// an owner does not have are skipped. A setting of a named owner that the blob does not hold takes its default. All or
/// nothing: if one value is refused, no value changes. A setting of [`NOT_SAVED`] is refused.
pub(crate) fn import(sh: &Shared, bytes: &[u8]) -> Result<(), EngineError> {
    let owners = settings_blob::decode(bytes).map_err(|e| match e {
        BlobError::NotSettings => ImportError::NotSettings,
        BlobError::Version(v) => ImportError::Version {
            found: u64::from(v),
        },
        BlobError::Damaged(at) => ImportError::Damaged { at },
    })?;
    let mut planned: Vec<(OwnerRef, String, Changes)> = Vec::new();
    for o in owners {
        if let Some((setting, _)) = o.entries.iter().find(|(k, _)| not_saved(&o.key, k)) {
            return Err(ImportError::NotSaved {
                owner: o.key.clone(),
                setting: setting.clone(),
            }
            .into());
        }
        let Ok(r) = sh.find(&o.key) else {
            continue;
        };
        let changes = sh.owner(r, |x| changes_of(x, &o.key, &o.entries))?;
        let changes = changes.map_err(|(setting, error)| ImportError::Value {
            owner: o.key.clone(),
            setting: Some(setting),
            error,
        })?;
        planned.push((r, o.key, changes));
    }
    for (r, key, changes) in planned {
        sh.owner(r, |o| {
            changes.iter().try_for_each(|(id, v)| {
                owner::apply(o, *id, v).map_err(|error| ImportError::Apply {
                    owner: key.clone(),
                    text: owner::setting_error_text(o.schema(), "setting", &error),
                    error,
                })
            })
        })??;
    }
    Ok(())
}

/// The new values of the settings that a blob changes for one owner.
type Changes = Vec<(SettingId, Value)>;

/// The settings of `o` (owner key `key`) that `entries` change, checked against its schema. Every setting that can be
/// saved starts at its default; a fixed setting, a setting of [`NOT_SAVED`] and a setting without a default keep their
/// value. On a refusal, returns the setting key and the error.
fn changes_of(
    o: &mut dyn Owner,
    key: &str,
    entries: &[(String, Stored)],
) -> Result<Changes, (String, ValuesError)> {
    let schema = Arc::clone(o.schema());
    let now = o.values();
    let mut next: Vec<Option<Value>> = schema
        .settings()
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let kept = s.fixed || s.default.is_none() || not_saved(key, &s.field.key);
            if kept {
                now.and_then(|v| v.value(SettingId(i as u16))).cloned()
            } else {
                Some(s.default.clone())
            }
        })
        .collect();
    for (k, stored) in entries {
        let Ok(id) = schema.id(k) else {
            continue;
        };
        let s = &schema.settings()[id.index()];
        if s.fixed {
            continue;
        }
        let refused = |e: ValuesError| (k.clone(), e);
        let value = match stored {
            Stored::Value(v) => v.clone(),
            Stored::Choice(c) => Value::Choice(
                s.field
                    .choice_index(c)
                    .ok_or_else(|| refused(ValuesError::UnknownChoice { id, key: c.clone() }))?,
            ),
            Stored::Unknown(_) => continue,
        };
        schema
            .check(id, &value)
            .map_err(|e| refused(ValuesError::Setting(e)))?;
        next[id.index()] = Some(value);
    }
    let mut out = Vec::new();
    for (i, new) in next.into_iter().enumerate() {
        let id = SettingId(i as u16);
        if let Some(v) = new {
            if Some(&v) != now.and_then(|x| x.value(id)) {
                out.push((id, v));
            }
        }
    }
    Ok(out)
}

/// The JSON Schema (draft 2020-12) of `schema`: one property per setting that is not fixed, with its title, help, unit, type,
/// range, step, choices and default.
pub(crate) fn json_schema(schema: &Schema) -> String {
    let defaults = Values::defaults(&std::sync::Arc::new(schema.clone())).to_json();
    let mut props = Map::new();
    for s in schema.settings() {
        if s.fixed {
            continue;
        }
        let f = &s.field;
        let mut p = Map::new();
        p.insert("title".into(), json!(f.label));
        if !s.help.is_empty() {
            p.insert("description".into(), json!(s.help));
        }
        if !f.unit.is_empty() {
            p.insert("x-unit".into(), json!(f.unit));
        }
        let ty = match f.kind {
            ValueKind::Bool => "boolean",
            ValueKind::I64 => "integer",
            ValueKind::Text | ValueKind::Choice => "string",
            ValueKind::Position => "object",
            ValueKind::F64 | ValueKind::Hz | ValueKind::Time | ValueKind::Duration => "number",
            ValueKind::Bytes | ValueKind::Any | ValueKind::Flags => continue,
        };
        p.insert("type".into(), json!(ty));
        if f.kind == ValueKind::Choice {
            let keys: Vec<&str> = f.choices.iter().map(|c| c.key.as_ref()).collect();
            p.insert("enum".into(), json!(keys));
        }
        if f.kind == ValueKind::Position {
            p.insert(
                "properties".into(),
                json!({ "lat": { "type": "number" }, "lon": { "type": "number" } }),
            );
            p.insert("required".into(), json!(["lat", "lon"]));
        }
        if let Some(r) = &s.range {
            p.insert("minimum".into(), json!(r.min));
            p.insert("maximum".into(), json!(r.max));
            if r.step > 0.0 {
                p.insert("multipleOf".into(), json!(r.step));
            }
        }
        if let Some(d) = defaults.get(f.key.as_ref()) {
            p.insert("default".into(), d.clone());
        }
        props.insert(f.key.to_string(), Json::Object(p));
    }
    let doc = json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "type": "object",
        "additionalProperties": false,
        "properties": props,
    });
    format!("{doc:#}")
}

/// A saved state: the radios by their address, the sessions in their order, and the services by their owner key.
#[derive(Serialize, Deserialize)]
struct StateFile {
    version: u64,
    radios: Vec<SavedRadio>,
    sessions: Vec<SavedSession>,
    services: BTreeMap<String, StateBlob>,
}

/// A radio of a saved state.
#[derive(Serialize, Deserialize)]
struct SavedRadio {
    driver: String,
    address: String,
    settings: Json,
    state: Option<StateBlob>,
}

/// A session of a saved state: a stack on a radio.
#[derive(Serialize, Deserialize)]
struct SavedSession {
    stack: String,
    submode: String,
    freq_hz: f64,
    radio: String,
    state: Option<StateBlob>,
}

/// The saved state of the engine: every open radio, every session of a stack on a radio that the host started, and the
/// services that keep a state, as the bytes of a state file. It does not lock the engine owner.
pub(crate) fn save_state(sh: &Shared) -> Result<Vec<u8>, EngineError> {
    let radios = lock(&sh.radios)
        .iter()
        .map(|(_, r)| Arc::clone(r))
        .collect::<Vec<_>>();
    let radios = radios
        .iter()
        .map(|r| {
            let o = r.owner.lock();
            SavedRadio {
                driver: r.driver.to_owned(),
                address: r.address.clone(),
                settings: o.values().map_or(Json::Null, Values::to_json),
                state: o.save_state(),
            }
        })
        .collect();
    let mut sessions = Vec::new();
    for (_, s) in sh.saved_sessions() {
        let Input::Radio(r) = s.input else {
            continue;
        };
        let (Some(radio), Some((stack, submode, freq_hz))) = (sh.radio(r), {
            let c = s.owner();
            c.stack.map(|d| {
                let freq = c.values().map_or(0.0, |v| v.get(&TUNED));
                (d.id, c.submode.clone().unwrap_or_default(), freq)
            })
        }) else {
            continue;
        };
        sessions.push(SavedSession {
            stack: stack.to_owned(),
            submode,
            freq_hz,
            radio: radio.address.clone(),
            state: s
                .link
                .with_run(|run| {
                    // A saved state needs every component on this thread.
                    if let Some(st) = run.rx.staged() {
                        st.gather();
                    }
                    run.rx.save_state()
                })
                .flatten(),
        });
    }
    let mut services = BTreeMap::new();
    services.insert(
        ServiceKind::Bank.key().to_owned(),
        sh.services.bank.lock().save_state()?,
    );
    let file = StateFile {
        version: VERSION,
        radios,
        sessions,
        services,
    };
    serde_json::to_vec(&file).map_err(|_| EngineError::Encode)
}

/// Restores the saved state `bytes` that [`save_state`] returned: opens each radio that is not open at its address, starts each
/// session that does not run in its place in the order, and restores the saved state of each. `e` is the engine owner, which
/// the caller holds.
///
/// # Errors
///
/// [`EngineError::Import`] if the bytes are not a state file. [`EngineError::Restore`] with the first radio, session or
/// service that was not restored. What came before it stays restored.
pub(crate) fn restore_state(
    sh: &Shared,
    e: &mut EngineOwner,
    bytes: &[u8],
) -> Result<(), EngineError> {
    let file: StateFile = serde_json::from_slice(bytes).map_err(|err| {
        if err.is_data() {
            EngineError::from(ImportError::NotSettings)
        } else {
            ImportError::NotJson {
                line: err.line(),
                column: err.column(),
            }
            .into()
        }
    })?;
    if file.version != VERSION {
        return Err(ImportError::Version {
            found: file.version,
        }
        .into());
    }
    for r in &file.radios {
        sh.restore_radio(e, r)
            .map_err(|source| EngineError::Restore {
                item: RestoreItem::Radio {
                    address: r.address.clone(),
                },
                source: Box::new(source),
            })?;
    }
    let running = sh.saved_sessions();
    for (order, s) in file.sessions.iter().enumerate() {
        sh.restore_session(e, s, running.get(order))
            .map_err(|source| EngineError::Restore {
                item: RestoreItem::Session { order },
                source: Box::new(source),
            })?;
    }
    for (key, blob) in &file.services {
        let refused = |source| EngineError::Restore {
            item: RestoreItem::Service { key: key.clone() },
            source: Box::new(source),
        };
        if key != ServiceKind::Bank.key() {
            return Err(refused(EngineError::NoSuchKey { key: key.clone() }));
        }
        sh.services
            .bank
            .lock()
            .restore_state(blob)
            .map_err(|x| refused(x.into()))?;
    }
    Ok(())
}

impl Shared {
    /// The sessions that a saved state holds, in their order: the sessions of a stack on a radio that the host started.
    fn saved_sessions(&self) -> Vec<(SessionId, Arc<Session>)> {
        lock(&self.sessions)
            .iter()
            .map(|(h, s)| (SessionId::new(h), Arc::clone(s)))
            .filter(|(_, s)| {
                s.by == By::Host
                    && matches!(s.input, Input::Radio(_))
                    && lock(&s.control).stack.is_some()
            })
            .collect()
    }

    /// Opens the radio of `r` if no radio is open at its address with its driver, and sets its settings and state.
    fn restore_radio(&self, e: &mut EngineOwner, r: &SavedRadio) -> Result<(), EngineError> {
        let open = lock(&self.radios)
            .iter()
            .find(|(_, x)| x.address == r.address && x.driver == r.driver)
            .map(|(h, _)| RadioId::new(h));
        let id = match open {
            Some(id) => id,
            None => {
                let kind = self.find(&format!("driver.{}", r.driver)).map_err(|_| {
                    EngineError::NoSuchDriver {
                        key: r.driver.clone(),
                    }
                })?;
                let mut values = self.owner(kind, |o| Values::defaults(o.schema()))?;
                if !r.settings.is_null() {
                    values
                        .update_from_json(&r.settings)
                        .map_err(|error| ImportError::Value {
                            owner: format!("driver.{}", r.driver),
                            setting: None,
                            error,
                        })?;
                }
                self.open_radio(Some(e), &r.driver, &r.address, &values)?
            }
        };
        if let Some(blob) = &r.state {
            let x = self
                .radio(id)
                .ok_or(EngineError::NoSuchOwner(OwnerRef::Radio(id)))?;
            x.owner.lock().restore_state(blob)?;
        }
        Ok(())
    }

    /// Starts the session of `s`, unless `running`, the session in its place in the order, runs its stack on its radio. Tunes
    /// it and sets its state.
    fn restore_session(
        &self,
        e: &mut EngineOwner,
        s: &SavedSession,
        running: Option<&(SessionId, Arc<Session>)>,
    ) -> Result<(), EngineError> {
        let radio = lock(&self.radios)
            .iter()
            .find(|(_, x)| x.address == s.radio)
            .map(|(h, _)| RadioId::new(h))
            .ok_or_else(|| EngineError::NoSuchKey {
                key: s.radio.clone(),
            })?;
        let desc = self.stack(&s.stack)?;
        let same = running.filter(|(_, x)| {
            x.input == Input::Radio(radio)
                && lock(&x.control).stack.is_some_and(|d| d.id == desc.id)
        });
        let id = match same {
            Some((id, _)) => *id,
            None => {
                let sub = (!s.submode.is_empty()).then_some(s.submode.as_str());
                let values = Values::defaults(&(desc.settings)());
                let rx = self.make_receiver(desc, sub, &values, &e.station())?;
                self.start(Some(e), By::Host, rx, Input::Radio(radio), Some(desc), sub)?
            }
        };
        self.tune_to(id, s.freq_hz)?;
        if let Some(blob) = &s.state {
            let x = self
                .session(id)
                .ok_or(EngineError::NoSuchOwner(OwnerRef::Session(id)))?;
            let blob = blob.clone();
            x.link
                .with_run(move |run| {
                    let r = run.rx.restore_state(&blob);
                    run.touched();
                    r
                })
                .ok_or(EngineError::Stopped)??;
        }
        Ok(())
    }
}
