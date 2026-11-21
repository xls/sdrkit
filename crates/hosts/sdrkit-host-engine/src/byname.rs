//! Calls by name. A host that knows what it wants opens a radio by its source text, starts a stack by its id, sets a
//! setting by its key and runs a command by its key. Each call uses the declarations and defaults of the owner, so a
//! stack works without any setting. The calls by id stay for a host that reads the declarations.

use crate::{Engine, EngineError, Input, OwnerRef, RadioId, SessionId};
use sdrkit_core::{Record, SettingDecl, SettingError, SettingId, Value, ValueKind, Values};
use std::sync::Arc;

impl From<RadioId> for OwnerRef {
    fn from(r: RadioId) -> Self {
        OwnerRef::Radio(r)
    }
}

impl From<SessionId> for OwnerRef {
    fn from(s: SessionId) -> Self {
        OwnerRef::Session(s)
    }
}

/// `value` in the kind that `decl` takes: a text that is the key or the label of a choice becomes that choice, and a
/// number becomes the number kind of the setting. Another value is returned as it is, and the check of the schema judges
/// it.
fn fit(decl: &SettingDecl, id: SettingId, value: Value) -> Result<Value, SettingError> {
    let number = match &value {
        Value::F64(x) | Value::Hz(x) | Value::Time(x) | Value::Duration(x) => Some(*x),
        Value::I64(n) => Some(*n as f64),
        _ => None,
    };
    Ok(match (decl.field.kind, &value, number) {
        (ValueKind::Choice, Value::Text(t), _) => {
            let by_label = || {
                decl.field
                    .choices
                    .iter()
                    .position(|c| c.label.eq_ignore_ascii_case(t))
                    .and_then(|i| u16::try_from(i).ok())
            };
            match decl.field.choice_index(t).or_else(by_label) {
                Some(i) => Value::Choice(i),
                None => {
                    return Err(SettingError::NotAChoice {
                        id,
                        index: u16::MAX,
                    })
                }
            }
        }
        (ValueKind::F64, _, Some(x)) => Value::F64(x),
        (ValueKind::Hz, _, Some(x)) => Value::Hz(x),
        (ValueKind::Time, _, Some(x)) => Value::Time(x),
        (ValueKind::Duration, _, Some(x)) => Value::Duration(x),
        (ValueKind::I64, _, Some(x)) if x.fract() == 0.0 => Value::I64(x as i64),
        _ => value,
    })
}

/// `text` as a value of the kind that `decl` takes: `yes` or `no` for a switch, a number, the key or the label of a
/// choice, or the text itself.
fn from_text(decl: &SettingDecl, id: SettingId, text: &str) -> Result<Value, SettingError> {
    let t = text.trim();
    let wrong = || SettingError::Malformed { id, at: None };
    let number = || t.parse::<f64>().map_err(|_| wrong());
    Ok(match decl.field.kind {
        ValueKind::Bool => match t.to_ascii_lowercase().as_str() {
            "yes" | "on" | "true" | "1" => Value::Bool(true),
            "no" | "off" | "false" | "0" => Value::Bool(false),
            _ => return Err(wrong()),
        },
        ValueKind::I64 => Value::I64(t.parse().map_err(|_| wrong())?),
        ValueKind::F64 => Value::F64(number()?),
        ValueKind::Hz => Value::Hz(number()?),
        ValueKind::Time => Value::Time(number()?),
        ValueKind::Duration => Value::Duration(number()?),
        _ => fit(decl, id, Value::Text(text.to_owned()))?,
    })
}

impl Engine {
    /// As [`Engine::set`], with the value as a text: `yes` or `no` for a switch, a number, the key or the label of a
    /// choice.
    ///
    /// # Errors
    ///
    /// Those of [`Engine::set`]. A text that is not a value of the setting is [`SettingError::Malformed`].
    pub fn set_text(
        &self,
        owner: impl Into<OwnerRef>,
        key: &str,
        text: &str,
    ) -> Result<(), EngineError> {
        self.owner(owner.into(), |o| {
            let schema = Arc::clone(o.schema());
            let id = EngineError::id_of(&schema, key)?;
            let decl = schema
                .setting(id)
                .ok_or_else(|| EngineError::setting(&schema, SettingError::NoSuchId { id }))?;
            let value = from_text(decl, id, text).map_err(|e| EngineError::setting(&schema, e))?;
            EngineError::apply(o, id, &value)
        })?
    }

    /// As [`Engine::call`], with each argument as a text ([`Engine::set_text`]).
    ///
    /// # Errors
    ///
    /// Those of [`Engine::call`].
    pub fn call_text(
        &self,
        owner: impl Into<OwnerRef>,
        key: &str,
        args: &[(&str, &str)],
    ) -> Result<Option<Record>, EngineError> {
        let owner = owner.into();
        let values = self.owner(owner, |o| {
            let id = o
                .commands()
                .id(key)
                .map_err(|_| EngineError::no_command(key))?;
            let def = o
                .commands()
                .def(id)
                .ok_or_else(|| EngineError::no_command(key))?;
            let bad = |arg, error| {
                EngineError::command(
                    &*o,
                    id,
                    sdrkit_core::CommandError::BadArgument { arg, error },
                )
            };
            let mut out = Vec::with_capacity(args.len());
            for (k, text) in args {
                let arg = def.args.id(k).map_err(|e| bad(SettingId(0), e))?;
                let decl = def
                    .args
                    .setting(arg)
                    .ok_or_else(|| bad(arg, SettingError::NoSuchId { id: arg }))?;
                out.push((*k, from_text(decl, arg, text).map_err(|e| bad(arg, e))?));
            }
            Ok::<_, EngineError>(out)
        })??;
        self.call(owner, key, &values)
    }

    /// Opens the radio of `source`, written `<driver>:<address>` (`iiod:192.168.2.1`, `rtl_tcp:host:1234`, `synth:fm`),
    /// with the defaults of the driver.
    ///
    /// # Errors
    ///
    /// [`EngineError::NoSuchKey`] if there is no such driver, else what [`Engine::open_radio`] returns.
    pub fn open(&self, source: &str) -> Result<RadioId, EngineError> {
        let (driver, address) = source.split_once(':').unwrap_or((source, ""));
        let kind = self.find(&format!("driver.{driver}"))?;
        let values = self.owner(kind, |o| Values::defaults(o.schema()))?;
        self.open_radio(driver, address, &values)
    }

    /// Starts stack `stack` on `radio` with the defaults of the stack, on the default frequency of the stack.
    /// `freq_hz` is another frequency to start on.
    ///
    /// # Errors
    ///
    /// [`EngineError::NoSuchStack`] if there is no such stack, else what [`Engine::start_stack`] returns.
    pub fn start(
        &self,
        stack: &str,
        radio: RadioId,
        freq_hz: Option<f64>,
    ) -> Result<SessionId, EngineError> {
        let kind = self.find(&format!("stack.{stack}"))?;
        let values = self.owner(kind, |o| Values::defaults(o.schema()))?;
        self.start_stack(stack, None, &values, Input::Radio(radio), freq_hz)
    }

    /// Sets setting `key` of `owner` to `value`. A choice is set by the key or the label of the choice, as a text.
    ///
    /// # Errors
    ///
    /// [`EngineError::NoSuchOwner`], or [`EngineError::Setting`] with a sentence that names the setting: there is no
    /// such key, or the value is refused.
    pub fn set(
        &self,
        owner: impl Into<OwnerRef>,
        key: &str,
        value: impl Into<Value>,
    ) -> Result<(), EngineError> {
        let value = value.into();
        self.owner(owner.into(), |o| {
            let schema = Arc::clone(o.schema());
            let id = EngineError::id_of(&schema, key)?;
            let decl = schema
                .setting(id)
                .ok_or_else(|| EngineError::setting(&schema, SettingError::NoSuchId { id }))?;
            let value = fit(decl, id, value).map_err(|e| EngineError::setting(&schema, e))?;
            EngineError::apply(o, id, &value)
        })?
    }

    /// Runs command `key` of `owner` with the arguments `args` (key and value). An argument that is not given takes
    /// its default. Returns the reply of the command, if it has one.
    ///
    /// # Errors
    ///
    /// [`EngineError::NoSuchOwner`], or [`EngineError::Command`] with a sentence that names the command and the
    /// argument: there is no such command or argument, an argument is missing or refused, or the command fails.
    pub fn call(
        &self,
        owner: impl Into<OwnerRef>,
        key: &str,
        args: &[(&str, Value)],
    ) -> Result<Option<Record>, EngineError> {
        self.owner(owner.into(), |o| {
            let id = o
                .commands()
                .id(key)
                .map_err(|_| EngineError::no_command(key))?;
            let def = o
                .commands()
                .def(id)
                .ok_or_else(|| EngineError::no_command(key))?
                .clone();
            let mut given = def.new_args();
            for (k, v) in args {
                let refuse = |e| {
                    EngineError::command(
                        &*o,
                        id,
                        sdrkit_core::CommandError::BadArgument {
                            arg: SettingId(0),
                            error: e,
                        },
                    )
                };
                let arg = def.args.id(k).map_err(refuse)?;
                let decl = def.args.setting(arg).ok_or_else(|| {
                    EngineError::command(&*o, id, sdrkit_core::CommandError::NoSuchKey { arg })
                })?;
                let v = fit(decl, arg, v.clone()).map_err(|e| {
                    EngineError::command(
                        &*o,
                        id,
                        sdrkit_core::CommandError::BadArgument { arg, error: e },
                    )
                })?;
                given
                    .set_value(sdrkit_core::FieldId(arg.0), &v)
                    .map_err(|_| {
                        EngineError::command(
                            &*o,
                            id,
                            sdrkit_core::CommandError::BadArgument {
                                arg,
                                error: SettingError::WrongKind {
                                    id: arg,
                                    expected: decl.field.kind,
                                    got: v.kind().unwrap_or(decl.field.kind),
                                },
                            },
                        )
                    })?;
            }
            EngineError::invoke(o, id, &given)
        })?
    }

    /// What is wrong with session `id` now, as sentences joined by "; ": the problem of its receiver, the last refused
    /// request, and what stops it from transmitting. `None` if nothing is wrong.
    ///
    /// # Errors
    ///
    /// [`EngineError::NoSuchOwner`] if there is no such session.
    pub fn problem(&self, id: SessionId) -> Result<Option<String>, EngineError> {
        self.owner(OwnerRef::Session(id), |o| {
            let f = o.results().fields();
            let all: Vec<&str> = [
                &crate::session::PROBLEM,
                &crate::session::REQUEST_PROBLEM,
                &crate::transmit::TX_BLOCKED,
            ]
            .into_iter()
            .filter_map(|k| f.get(k))
            .filter(|t| !t.is_empty())
            .collect();
            (!all.is_empty()).then(|| all.join("; "))
        })
    }
}
