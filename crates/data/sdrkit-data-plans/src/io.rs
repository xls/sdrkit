//! Reads and writes one plan in the plan view, as CSV or as `ChannelPlan` JSON, for files made by hand.
//! The plan data format of [`crate::data`] is the main format for import and export.
//!
//! JSON here is a `ChannelPlan` object or an array of them. CSV is one plan per file: `# key: value` lines set the plan's id, name,
//! kind, countries, ITU regions, stacks and note, then a header row and one row per channel. The header names the columns, in any
//! order: `band`, `name`, `centre_mhz` (or `centre_khz`, `centre_hz`, `frequency`), `width_mhz` (or `width_khz`, `width_hz`),
//! `reallocated`, `pair_mhz`, `note`. A file with no header is read as `band,name,centre_mhz,width_mhz`. Rows of segments use
//! `lo_mhz` and `hi_mhz` (and `step_khz`, `first_mhz`, `modulation`) instead of a centre; a later header row starts a new table.

use crate::model::{Channel, ChannelPlan, PlanKind, Review, Segment};
use crate::registry::PlanError;
use sdrkit_core::Choice;
use sdrkit_util_text::csv;

/// Plans from JSON: one plan or an array.
pub fn from_json(text: &str) -> Result<Vec<ChannelPlan>, PlanError> {
    let v: serde_json::Value = serde_json::from_str(text).map_err(|e| PlanError::from(&e))?;
    let plans = if v.is_array() {
        serde_json::from_value::<Vec<ChannelPlan>>(v)
    } else {
        serde_json::from_value::<ChannelPlan>(v).map(|p| vec![p])
    };
    plans.map_err(|e| PlanError::from(&e))
}

/// Plans as pretty JSON (an array).
pub fn to_json(plans: &[&ChannelPlan]) -> String {
    serde_json::to_string_pretty(plans).unwrap_or_else(|_| "[]".into())
}

fn parse_bool(s: &str) -> bool {
    matches!(
        s.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "y" | "x"
    )
}

/// A frequency in the column's unit, or with a unit written after it ("474 MHz").
fn parse_freq(s: &str, unit_hz: f64) -> Option<f64> {
    let t = s.trim().to_ascii_lowercase();
    let (num, mult) = if let Some(n) = t.strip_suffix("ghz") {
        (n, 1e9)
    } else if let Some(n) = t.strip_suffix("mhz") {
        (n, 1e6)
    } else if let Some(n) = t.strip_suffix("khz") {
        (n, 1e3)
    } else if let Some(n) = t.strip_suffix("hz") {
        (n, 1.0)
    } else {
        (t.as_str(), unit_hz)
    };
    num.trim().parse::<f64>().ok().map(|v| v * mult)
}

/// One plan from CSV. `id` and `name` are used when the file does not give them.
pub fn from_csv(text: &str, id: &str, name: &str) -> Result<ChannelPlan, PlanError> {
    let mut plan = ChannelPlan::new(id, name, PlanKind::Other, Review::User, "user file");
    let mut header: Option<Vec<String>> = None;
    let mut errors = Vec::new();
    for (i, raw) in text.lines().enumerate() {
        let line = raw.trim().trim_start_matches('\u{feff}');
        if line.is_empty() {
            continue;
        }
        if let Some(meta) = line.strip_prefix('#') {
            if let Some((k, v)) = meta.split_once(':') {
                let v = v.trim();
                match k.trim().to_ascii_lowercase().as_str() {
                    "id" => plan.id = v.into(),
                    "name" => plan.name = v.into(),
                    "kind" => plan.kind = PlanKind::from_id(v).unwrap_or(PlanKind::Other),
                    "countries" => {
                        plan.countries = v
                            .split([' ', ';', ','])
                            .filter(|s| !s.is_empty())
                            .map(|s| s.to_ascii_uppercase())
                            .collect()
                    }
                    "itu_regions" => {
                        plan.itu_regions = v
                            .split([' ', ';', ','])
                            .filter_map(|s| s.parse().ok())
                            .collect()
                    }
                    "modes" => {
                        plan.modes = v
                            .split([' ', ';', ','])
                            .filter(|s| !s.is_empty())
                            .map(String::from)
                            .collect()
                    }
                    "deemphasis_us" => plan.deemphasis_s = v.parse::<f64>().ok().map(|u| u * 1e-6),
                    "note" => plan.note = v.into(),
                    "source" => plan.source = v.into(),
                    _ => {}
                }
            }
            continue;
        }
        let f = csv::split_line(line, ',');
        let looks_like_header = f.iter().any(|x| {
            let x = x.to_ascii_lowercase();
            x.contains("name") || x.contains("centre") || x.contains("center") || x.contains("lo_")
        }) && f.iter().all(|x| x.parse::<f64>().is_err());
        if looks_like_header {
            header = Some(f);
            continue;
        }
        let h = header.get_or_insert_with(|| {
            ["band", "name", "centre_mhz", "width_mhz"]
                .map(String::from)
                .to_vec()
        });
        let get = |names: &[&str]| {
            csv::column(h, names)
                .and_then(|c| f.get(c))
                .map(|s| s.as_str())
        };
        let freq = |names_units: &[(&str, f64)]| {
            names_units.iter().find_map(|(n, u)| {
                let v = get(&[n])?;
                if v.trim().is_empty() {
                    return None;
                }
                parse_freq(v, *u)
            })
        };
        let line_no = i + 1;
        let centre = freq(&[
            ("centre_mhz", 1e6),
            ("center_mhz", 1e6),
            ("centre_khz", 1e3),
            ("centre_hz", 1.0),
            ("frequency", 1e6),
            ("frequency_mhz", 1e6),
        ]);
        let lo = freq(&[("lo_mhz", 1e6), ("lo_khz", 1e3), ("lo_hz", 1.0)]);
        let hi = freq(&[("hi_mhz", 1e6), ("hi_khz", 1e3), ("hi_hz", 1.0)]);
        let label = get(&["name", "label", "channel"]).unwrap_or("").to_string();
        let note = get(&["note", "notes"]).unwrap_or("").to_string();
        if let (Some(lo), Some(hi)) = (lo, hi) {
            let mut s = Segment::new(label, lo, hi).note(note);
            if let Some(step) = freq(&[("step_khz", 1e3), ("step_hz", 1.0)]) {
                let first = freq(&[("first_mhz", 1e6), ("first_hz", 1.0)]).unwrap_or(lo);
                s = s.raster(first, step);
            }
            if let Some(m) = get(&["modulation", "mode"]) {
                s = s.modulation(m);
            }
            plan.segments.push(s);
            continue;
        }
        let Some(centre) = centre else {
            errors.push(format!("line {line_no}: no centre frequency"));
            continue;
        };
        let width = freq(&[
            ("width_mhz", 1e6),
            ("bandwidth_mhz", 1e6),
            ("width_khz", 1e3),
            ("width_hz", 1.0),
        ]);
        let Some(width) = width else {
            errors.push(format!("line {line_no}: no width"));
            continue;
        };
        let mut c = Channel::new(label, centre, width).note(note);
        c.reallocated = get(&["reallocated"]).map(parse_bool).unwrap_or(false);
        c.pair_hz = freq(&[("pair_mhz", 1e6), ("pair_hz", 1.0)]);
        if c.note.is_empty() {
            if let Some(b) = get(&["band"]) {
                c.note = b.to_string();
            }
        }
        plan.channels.push(c);
    }
    if !errors.is_empty() {
        return Err(PlanError::Invalid(errors));
    }
    crate::registry::validate(&plan)?;
    Ok(plan)
}

/// One plan as CSV: the `#` lines, then channels (`band,name,centre_mhz,width_mhz,reallocated,pair_mhz,note`), then segments
/// (`name,lo_mhz,hi_mhz,step_khz,first_mhz,modulation,note`) after a second header.
pub fn to_csv(plan: &ChannelPlan) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "# id: {}\n# name: {}\n# kind: {}\n",
        plan.id,
        plan.name.replace('\n', " "),
        plan.kind.key()
    ));
    if !plan.countries.is_empty() {
        out.push_str(&format!("# countries: {}\n", plan.countries.join(" ")));
    }
    if !plan.itu_regions.is_empty() {
        let r: Vec<String> = plan.itu_regions.iter().map(|r| r.to_string()).collect();
        out.push_str(&format!("# itu_regions: {}\n", r.join(" ")));
    }
    if !plan.modes.is_empty() {
        out.push_str(&format!("# modes: {}\n", plan.modes.join(" ")));
    }
    if let Some(d) = plan.deemphasis_s {
        out.push_str(&format!("# deemphasis_us: {}\n", csv::number(d * 1e6)));
    }
    if !plan.source.is_empty() {
        out.push_str(&format!("# source: {}\n", plan.source.replace('\n', " ")));
    }
    if !plan.note.is_empty() {
        out.push_str(&format!("# note: {}\n", plan.note.replace('\n', " ")));
    }
    if !plan.channels.is_empty() {
        out.push_str("band,name,centre_mhz,width_mhz,reallocated,pair_mhz,note\n");
        for c in &plan.channels {
            out.push_str(&csv::record(
                &[
                    String::new(),
                    c.name.clone(),
                    csv::number(c.centre_hz / 1e6),
                    csv::number(c.width_hz / 1e6),
                    if c.reallocated {
                        "1".into()
                    } else {
                        String::new()
                    },
                    c.pair_hz.map(|p| csv::number(p / 1e6)).unwrap_or_default(),
                    c.note.clone(),
                ],
                ',',
            ));
            out.push('\n');
        }
    }
    if !plan.segments.is_empty() {
        out.push_str("name,lo_mhz,hi_mhz,step_khz,first_mhz,modulation,note\n");
        for s in &plan.segments {
            out.push_str(&csv::record(
                &[
                    s.name.clone(),
                    csv::number(s.lo_hz / 1e6),
                    csv::number(s.hi_hz / 1e6),
                    s.step_hz.map(|v| csv::number(v / 1e3)).unwrap_or_default(),
                    s.first_hz.map(|v| csv::number(v / 1e6)).unwrap_or_default(),
                    s.modulation.clone(),
                    s.note.clone(),
                ],
                ',',
            ));
            out.push('\n');
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn four_column_layout() {
        let text = "UHF,21,474,8\nUHF,22,482,8\nVHF,5,177.5,7\n";
        let p = from_csv(text, "x", "X").unwrap();
        assert_eq!(p.channels.len(), 3);
        assert_eq!(p.channels[2].centre_hz, 177.5e6);
        assert_eq!(p.channels[0].note, "UHF");
        let text = "band,label,centre_mhz,bandwidth_mhz\nUHF,21,474,8\nUHF,bad,,8\n";
        assert!(
            matches!(from_csv(text, "x", "X"), Err(PlanError::Invalid(e)) if e[0].starts_with("line 3"))
        );
    }
}
