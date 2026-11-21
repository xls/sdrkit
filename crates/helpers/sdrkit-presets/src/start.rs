//! Starts a stack from a preset through the public calls of the engine.

use crate::data::Data;
use crate::error::StartError;
use crate::preset::{value_text, Options, Preset};
use crate::region::Region;
use sdrkit_core::{FieldDecl, SettingId, Value, ValueKind, Values};
use sdrkit_host_engine::{Engine, EngineError, Input, RadioId, SessionId};

/// `value` as a value of the kind that `field` takes: a text becomes the choice with that key or a number, a number
/// becomes the number kind of the field. `None` if the text is not such a value.
fn fit(field: &FieldDecl, value: &Value) -> Option<Value> {
    let number = match value {
        Value::F64(x) | Value::Hz(x) => Some(*x),
        Value::Text(t) => t.trim().parse::<f64>().ok().filter(|x| x.is_finite()),
        _ => None,
    };
    Some(match (field.kind, value) {
        (ValueKind::Choice, Value::Text(t)) => Value::Choice(field.choice_index(t)?),
        (ValueKind::F64, Value::Text(_) | Value::Hz(_)) => Value::F64(number?),
        (ValueKind::Hz, Value::Text(_) | Value::F64(_)) => Value::Hz(number?),
        (ValueKind::Bool, Value::Text(t)) => match t.trim() {
            "yes" | "true" => Value::Bool(true),
            "no" | "false" => Value::Bool(false),
            _ => return None,
        },
        _ => value.clone(),
    })
}

/// One setting of a preset in the form that its stack takes: the id of the setting in the schema of the stack, its key,
/// and the value in the kind of the setting (a choice as its index).
#[derive(Clone, Debug, PartialEq)]
pub struct PresetSetting {
    /// The id of the setting in the schema of the stack.
    pub id: SettingId,
    /// The key of the setting.
    pub key: String,
    /// The value, of the kind of the setting.
    pub value: Value,
}

/// The settings of `preset` as settings of its stack in `engine`, in the order of the preset, ready to set.
///
/// # Errors
///
/// [`StartError::Setting`] if a value of the preset is not a value of its setting, [`StartError::Engine`] if there is
/// no such stack or the stack has no setting of a key.
pub fn preset_settings(engine: &Engine, preset: &Preset) -> Result<Vec<PresetSetting>, StartError> {
    let kind = engine.find(&format!("stack.{}", preset.stack))?;
    let schema = engine.owner(kind, |o| std::sync::Arc::clone(o.schema()))?;
    preset
        .settings
        .iter()
        .map(|(key, value)| {
            let id = EngineError::id_of(&schema, key)?;
            let value = schema
                .setting(id)
                .and_then(|decl| fit(&decl.field, value))
                .ok_or_else(|| StartError::Setting {
                    stack: preset.stack.clone(),
                    key: key.clone(),
                    value: value_text(value),
                })?;
            Ok(PresetSetting {
                id,
                key: key.clone(),
                value,
            })
        })
        .collect()
}

/// The settings of the stack of `preset` for a start: the defaults of the stack with [`preset_settings`].
///
/// # Errors
///
/// Those of [`preset_settings`], and [`StartError::Engine`] if the stack refuses a value.
pub fn preset_values(engine: &Engine, preset: &Preset) -> Result<Values, StartError> {
    let kind = engine.find(&format!("stack.{}", preset.stack))?;
    let mut values = engine.owner(kind, |o| Values::defaults(o.schema()))?;
    for s in preset_settings(engine, preset)? {
        EngineError::set(&mut values, s.id, &s.value)?;
    }
    Ok(values)
}

/// Starts the stack of `preset` on `radio` with the settings of [`preset_values`], tuned to the frequency of the
/// preset.
///
/// # Errors
///
/// Those of [`preset_values`], and [`StartError::Engine`] if the stack does not start.
pub fn start_preset(
    engine: &Engine,
    radio: RadioId,
    preset: &Preset,
) -> Result<SessionId, StartError> {
    let values = preset_values(engine, preset)?;
    Ok(engine.start_stack(
        &preset.stack,
        preset.submode.as_deref(),
        &values,
        Input::Radio(radio),
        Some(preset.frequency_hz),
    )?)
}

/// Starts `stack` on `radio` with the preset that `data` has for it in `region` and the choices of `options`
/// ([`Data::preset`], [`start_preset`]).
///
/// # Errors
///
/// [`StartError::Preset`] if the data has no preset, else those of [`start_preset`].
pub fn start(
    data: &Data,
    engine: &Engine,
    radio: RadioId,
    stack: &str,
    region: &Region,
    options: &Options,
) -> Result<SessionId, StartError> {
    let preset = data.preset(stack, region, options)?;
    start_preset(engine, radio, &preset)
}
