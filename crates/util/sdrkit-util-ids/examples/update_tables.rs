//! Rebuilds the data files that come from machine-readable public lists. Writes the new file to standard output.
//!
//! ```text
//! cargo run -p sdrkit-util-ids --example update_tables -- oui <oui.csv> <oui_keep.txt> <min assignments> > data/oui.tsv
//! cargo run -p sdrkit-util-ids --example update_tables -- bt-company <company_identifiers.yaml> > data/bt_company.tsv
//! cargo run -p sdrkit-util-ids --example update_tables -- bt-uuid <service_uuids.yaml> <member_uuids.yaml> > data/bt_uuid16.tsv
//! cargo run -p sdrkit-util-ids --example update_tables -- ant-manufacturer <profile.py> > data/ant_manufacturer.tsv
//! ```
//!
//! `oui`: keeps every MA-L assignment of each organisation that holds at least `<min assignments>` of them, and every assignment
//! of an organisation whose name contains (ignoring case) a line of `<oui_keep.txt>`. A threshold of 1 keeps the whole listing.
//! Rows named "Private" are left out.

use sdrkit_util_ids::table::{records, Separator};
use std::collections::{BTreeMap, HashMap};
use std::process::ExitCode;

fn read(path: &str) -> Result<String, String> {
    std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))
}

fn clean(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn oui(csv: &str, keep: &str, min: usize, source: &str) -> Result<String, String> {
    let keep: Vec<String> = keep
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(str::to_lowercase)
        .collect();
    let mut rows: Vec<(u32, String)> = Vec::new();
    for (i, r) in records(csv, Separator::Comma).enumerate() {
        if i == 0 && r.cell(0) == "Registry" {
            continue;
        }
        if r.cell(0) != "MA-L" {
            continue;
        }
        let key = u32::from_str_radix(r.cell(1), 16)
            .map_err(|_| format!("line {}: bad assignment", r.line))?;
        let name = clean(r.cell(2));
        if name.is_empty() || name.eq_ignore_ascii_case("private") {
            continue;
        }
        rows.push((key, name));
    }
    let total = rows.len();
    let mut count: HashMap<&str, usize> = HashMap::new();
    for (_, n) in &rows {
        *count.entry(n.as_str()).or_default() += 1;
    }
    let kept: BTreeMap<u32, &str> = rows
        .iter()
        .filter(|(_, n)| {
            count[n.as_str()] >= min || {
                let l = n.to_lowercase();
                keep.iter().any(|k| l.contains(k.as_str()))
            }
        })
        .map(|(k, n)| (*k, n.as_str()))
        .collect();
    let mut out = format!(
        "# IEEE OUI (MA-L) assignments: the organisation each three-byte prefix is assigned to.\n\
         # Source: IEEE Registration Authority, public MA-L listing https://standards-oui.ieee.org/oui/oui.csv ({source}).\n\
         # Subset: every assignment of each organisation holding at least {min} MA-L assignments, and every assignment of the\n\
         #   organisations matched by data/oui_keep.txt; {} of {total} assignments. Rows named \"Private\" are left out.\n\
         # Update: cargo run -p sdrkit-util-ids --example update_tables -- oui oui.csv data/oui_keep.txt {min} > data/oui.tsv\n\
         # Columns: OUI (six hex digits), organisation name as published\n",
        kept.len()
    );
    for (k, n) in kept {
        out.push_str(&format!("{k:06X}\t{n}\n"));
    }
    Ok(out)
}

/// The value of a `key: value` YAML line, with single or double quotes removed.
fn yaml_value(line: &str, key: &str) -> Option<String> {
    let t = line.trim_start().trim_start_matches("- ").trim_start();
    let v = t.strip_prefix(key)?.strip_prefix(':')?.trim();
    Some(
        if let Some(s) = v.strip_prefix('\'').and_then(|s| s.strip_suffix('\'')) {
            s.replace("''", "'")
        } else if let Some(s) = v.strip_prefix('"').and_then(|s| s.strip_suffix('"')) {
            s.replace("\\\"", "\"").replace("\\\\", "\\")
        } else {
            v.to_string()
        },
    )
}

fn hex16(v: &str) -> Result<u16, String> {
    u16::from_str_radix(v.trim_start_matches("0x").trim_start_matches("0X"), 16)
        .map_err(|_| format!("bad value {v:?}"))
}

/// (value, name) pairs of a SIG assigned-numbers YAML list.
fn yaml_pairs(text: &str, key: &str) -> Result<Vec<(u16, String)>, String> {
    let mut out = Vec::new();
    let mut cur: Option<u16> = None;
    for line in text.lines() {
        if line.trim_start().starts_with('#') {
            continue;
        }
        if let Some(v) = yaml_value(line, key) {
            cur = Some(hex16(&v)?);
        } else if let (Some(n), Some(k)) = (yaml_value(line, "name"), cur) {
            out.push((k, clean(&n)));
            cur = None;
        }
    }
    Ok(out)
}

fn bt_company(yaml: &str, source: &str) -> Result<String, String> {
    let mut v = yaml_pairs(yaml, "value")?;
    v.sort();
    let mut out = format!(
        "# Bluetooth company identifiers (manufacturer specific data, Core Specification Supplement Part A clause 1.4).\n\
         # Source: Bluetooth SIG Assigned Numbers, company_identifiers/company_identifiers.yaml,\n\
         #   https://bitbucket.org/bluetooth-SIG/public/src/main/assigned_numbers/ ({source}). The whole list.\n\
         # Update: cargo run -p sdrkit-util-ids --example update_tables -- bt-company company_identifiers.yaml > data/bt_company.tsv\n\
         # Columns: identifier (hex), company name as published\n"
    );
    for (k, n) in v {
        out.push_str(&format!("{k:04X}\t{n}\n"));
    }
    Ok(out)
}

fn bt_uuid(service: &str, member: &str, source: &str) -> Result<String, String> {
    let mut v: Vec<(u16, &str, String)> = yaml_pairs(service, "uuid")?
        .into_iter()
        .map(|(k, n)| (k, "service", n))
        .chain(
            yaml_pairs(member, "uuid")?
                .into_iter()
                .map(|(k, n)| (k, "member", n)),
        )
        .collect();
    v.sort();
    let mut out = format!(
        "# Bluetooth 16-bit UUIDs: GATT services, and the UUIDs assigned to SIG member companies.\n\
         # Source: Bluetooth SIG Assigned Numbers, uuids/service_uuids.yaml and uuids/member_uuids.yaml,\n\
         #   https://bitbucket.org/bluetooth-SIG/public/src/main/assigned_numbers/ ({source}). The whole lists.\n\
         # Update: cargo run -p sdrkit-util-ids --example update_tables -- bt-uuid service_uuids.yaml member_uuids.yaml \\\n\
         #   > data/bt_uuid16.tsv\n\
         # Columns: UUID (hex), service or member, name as published\n"
    );
    for (k, kind, n) in v {
        out.push_str(&format!("{k:04X}\t{kind}\t{n}\n"));
    }
    Ok(out)
}

fn ant_manufacturer(profile: &str, source: &str) -> Result<String, String> {
    let start = profile
        .find("'manufacturer': {")
        .ok_or("no 'manufacturer' type in the FIT profile")?;
    let version = profile
        .lines()
        .find_map(|l| l.strip_prefix("# Profile Version = "))
        .unwrap_or("version not given")
        .trim();
    let mut v: Vec<(u16, String)> = Vec::new();
    for line in profile[start..].lines().skip(1) {
        let l = line.trim();
        if l.starts_with('}') {
            break;
        }
        let Some((id, rest)) = l.split_once(':') else {
            continue;
        };
        let id: u16 = id.trim().parse().map_err(|_| format!("bad id in {l:?}"))?;
        let name = rest
            .trim()
            .trim_start_matches('\'')
            .split('\'')
            .next()
            .unwrap_or("");
        if name.is_empty() {
            return Err(format!("no name in {l:?}"));
        }
        v.push((id, name.to_string()));
    }
    if v.is_empty() {
        return Err("the 'manufacturer' type is empty".into());
    }
    v.sort();
    let mut out = format!(
        "# ANT+ and FIT manufacturer identifiers (ANT+ common data page 80, the FIT `manufacturer` type).\n\
         # Source: Garmin FIT SDK, Profile Version {version}, type `manufacturer`, as garmin_fit_sdk/profile.py of\n\
         #   https://github.com/garmin/fit-python-sdk ({source}). The whole list.\n\
         # Update: cargo run -p sdrkit-util-ids --example update_tables -- ant-manufacturer profile.py > data/ant_manufacturer.tsv\n\
         # Columns: identifier (decimal), FIT name as published\n"
    );
    for (k, n) in v {
        out.push_str(&format!("{k}\t{n}\n"));
    }
    Ok(out)
}

fn run(args: &[String]) -> Result<String, String> {
    let source = std::env::var("SOURCE_DATE").unwrap_or_else(|_| "retrieval date not given".into());
    match args {
        [cmd, csv, keep, min] if cmd == "oui" => {
            let min = min.parse().map_err(|_| "the threshold is a number".to_string())?;
            oui(&read(csv)?, &read(keep)?, min, &source)
        }
        [cmd, yaml] if cmd == "bt-company" => bt_company(&read(yaml)?, &source),
        [cmd, s, m] if cmd == "bt-uuid" => bt_uuid(&read(s)?, &read(m)?, &source),
        [cmd, p] if cmd == "ant-manufacturer" => ant_manufacturer(&read(p)?, &source),
        _ => Err("usage: update_tables oui <oui.csv> <keep.txt> <min> | bt-company <yaml> | bt-uuid <service.yaml> <member.yaml> | ant-manufacturer <profile.py>\n\
                  (set SOURCE_DATE to the retrieval date to record it in the header)"
            .into()),
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(text) => {
            print!("{text}");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}
