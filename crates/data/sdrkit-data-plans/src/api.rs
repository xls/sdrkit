//! The import and export of plan files.

use crate::data::{self, PlanFile};
use crate::io;
use crate::registry::{PlanError, Plans};

/// The file format of a plan.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PlanFormat {
    /// Guessed from the text: JSON when it starts with `{` or `[`, CSV otherwise (export: CSV).
    #[default]
    Auto,
    /// CSV, one channel or segment per row.
    Csv,
    /// JSON, a list of plans.
    Json,
}

impl PlanFormat {
    /// The format named `text` ("json", "csv"; anything else is [`PlanFormat::Auto`]).
    pub fn from_name(text: &str) -> PlanFormat {
        match text.trim().to_ascii_lowercase().as_str() {
            "json" => PlanFormat::Json,
            "csv" => PlanFormat::Csv,
            _ => PlanFormat::Auto,
        }
    }
}

/// Imports user plans from a file's text. `format` is "json" or "csv" (empty: guessed from the first character). For CSV, `id`
/// and `name` are used when the file does not give them. Returns the ids added.
pub fn import(
    plans: &mut Plans,
    text: &str,
    format: &str,
    id: &str,
    name: &str,
) -> Result<Vec<String>, PlanError> {
    import_as(plans, text, PlanFormat::from_name(format), id, name)
}

/// Imports user plans from a file's text in `format` ([`PlanFormat::Auto`]: guessed from the first character). For CSV, `id` and
/// `name` are used when the file does not give them. Every plan is checked before any is added. Returns the ids added.
pub fn import_as(
    plans: &mut Plans,
    text: &str,
    format: PlanFormat,
    id: &str,
    name: &str,
) -> Result<Vec<String>, PlanError> {
    let json = match format {
        PlanFormat::Json => true,
        PlanFormat::Csv => false,
        PlanFormat::Auto => matches!(text.trim_start().chars().next(), Some('{') | Some('[')),
    };
    if json && is_plan_file(text) {
        let file =
            data::parse("import", text).map_err(|e| PlanError::Invalid(vec![e.to_string()]))?;
        return plans.add_file("import", file);
    }
    let list = if json {
        io::from_json(text)?
    } else {
        vec![io::from_csv(text, id, name)?]
    };
    // Check all before adding any.
    let mut trial = plans.clone();
    let mut ids = Vec::new();
    for p in list {
        ids.push(p.id.clone());
        trial.add_user(p)?;
    }
    *plans = trial;
    Ok(ids)
}

/// Exports plan `id` (loaded or user) as "json" or "csv"; `None` for an unknown id.
pub fn export(plans: &Plans, id: &str, format: &str) -> Option<String> {
    let f = if format.eq_ignore_ascii_case("csv") {
        PlanFormat::Csv
    } else {
        PlanFormat::Json
    };
    export_as(plans, id, f)
}

/// Exports plan `id` (loaded or user) in `format` ([`PlanFormat::Auto`] is CSV); `None` for an unknown id. JSON is a plan
/// file of the data format that holds the one group, with the bands of the groups it includes.
pub fn export_as(plans: &Plans, id: &str, format: PlanFormat) -> Option<String> {
    let p = plans.get(id)?;
    Some(match format {
        PlanFormat::Json => {
            let mut g = plans.group(id)?.group.clone();
            g.include.clear();
            let file = PlanFile {
                schema: String::new(),
                format: data::FORMAT_VERSION,
                groups: vec![g],
                ..PlanFile::default()
            };
            to_text(&file)
        }
        PlanFormat::Csv | PlanFormat::Auto => io::to_csv(p),
    })
}

/// True when `text` is a plan file of the data format (a JSON object with a `format` key), not plan-view JSON.
pub fn is_plan_file(text: &str) -> bool {
    serde_json::from_str::<serde_json::Value>(text)
        .ok()
        .is_some_and(|v| v.get("format").is_some())
}

/// A plan file as indented JSON with a final line break.
pub fn to_text(file: &PlanFile) -> String {
    let mut s = serde_json::to_string_pretty(file).unwrap_or_else(|_| "{}".into());
    s.push('\n');
    s
}

/// The user's part of `plans` as one plan file (what a host saves to keep it).
pub fn user_plans_json(plans: &Plans) -> String {
    to_text(&plans.user_file())
}
