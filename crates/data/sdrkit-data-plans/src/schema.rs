//! The JSON Schema of the plan data format, made from the model types of [`crate::data`]. `docs/schema/plans.json` holds the
//! same text, and a test fails when they differ.

use crate::data::{Naming, PlanKind, RdsVariant, Service, FORMAT_VERSION};
use sdrkit_core::Choice;
use serde_json::{json, Map, Value};

/// The JSON Schema dialect.
pub const DIALECT: &str = "https://json-schema.org/draft/2020-12/schema";

fn text(desc: &str) -> Value {
    json!({ "type": "string", "description": desc })
}

fn number(desc: &str) -> Value {
    json!({ "type": "number", "description": desc })
}

fn hz(desc: &str) -> Value {
    json!({ "type": "number", "exclusiveMinimum": 0, "maximum": 300e9, "description": desc })
}

fn texts(desc: &str) -> Value {
    json!({ "type": "array", "items": { "type": "string" }, "description": desc })
}

fn def_ref(name: &str, desc: &str) -> Value {
    json!({ "$ref": format!("#/$defs/{name}"), "description": desc })
}

fn list_of(name: &str, desc: &str) -> Value {
    json!({ "type": "array", "items": { "$ref": format!("#/$defs/{name}") }, "description": desc })
}

fn object(desc: &str, required: &[&str], props: Vec<(&str, Value)>) -> Value {
    let mut p = Map::new();
    for (k, v) in props {
        p.insert(k.to_string(), v);
    }
    json!({
        "type": "object",
        "description": desc,
        "additionalProperties": false,
        "required": required,
        "properties": p,
    })
}

fn one_of_ids(ids: &[&str], desc: &str) -> Value {
    json!({ "enum": ids, "description": desc })
}

/// The JSON Schema of a plan file, as a JSON value.
pub fn json_schema() -> Value {
    let kinds: Vec<&str> = PlanKind::ALL.iter().map(|k| k.key()).collect();
    let services: Vec<&str> = Service::ALL.iter().map(|s| s.id()).collect();
    let naming = |n: Naming| match n {
        Naming::Number => "number",
        Naming::Icao833 => "icao_8_33",
    };
    let rds = |r: RdsVariant| match r {
        RdsVariant::Rds => "rds",
        RdsVariant::Rbds => "rbds",
    };
    let id = json!({ "type": "string", "pattern": "^[a-z0-9_-]{1,64}$", "description": "1 to 64 lower-case letters, digits, '_' or '-'." });
    let mode_ids = json!({
        "type": "array",
        "items": { "type": "string", "pattern": "^[a-z0-9_-]{1,64}$" },
        "description": "The sdrkit stack ids that receive the band, in the order to try. Empty when no stack does.",
    });
    let mut defs = Map::new();
    defs.insert(
        "source".into(),
        object(
            "A published document that data comes from.",
            &["publisher", "title", "date"],
            vec![
                (
                    "publisher",
                    text("Who publishes it (\"ACMA\", \"ITU\", \"FCC\", \"Ofcom\", \"BNetzA\")."),
                ),
                ("title", text("Its title.")),
                (
                    "number",
                    text(
                        "Its number, edition or legal id (\"47 CFR 73.603\", \"ECC/DEC/(15)05\").",
                    ),
                ),
                ("date", text("Its date or version (\"2026-07\").")),
                (
                    "clause",
                    text("The clause, table or page the values come from."),
                ),
                ("url", text("Where to read it.")),
            ],
        ),
    );
    defs.insert(
        "analog".into(),
        object(
            "The parameters of an analogue TV system.",
            &["system", "colour", "vision_above_lower_hz", "sound_spacing_hz"],
            vec![
                ("system", text("The system letters (\"B/G\", \"I\", \"M\").")),
                ("colour", text("The colour systems used with it (\"PAL\", \"SECAM\", \"NTSC\").")),
                ("vision_above_lower_hz", number("The vision carrier above the lower channel edge, Hz.")),
                ("sound_spacing_hz", number("The main sound carrier above the vision carrier, Hz.")),
                ("am_sound", json!({ "type": "boolean", "description": "The sound is amplitude modulated (system L)." })),
            ],
        ),
    );
    defs.insert(
        "numbering".into(),
        object(
            "Numbered channels: channel n is centred on first_centre_hz + (n - first) * step_hz + offset_hz.",
            &["first", "last", "first_centre_hz", "step_hz"],
            vec![
                ("first", json!({ "type": "integer", "minimum": 0, "description": "The first channel number." })),
                ("last", json!({ "type": "integer", "minimum": 0, "description": "The last channel number." })),
                ("first_centre_hz", hz("The centre of the first channel, without offset_hz, Hz.")),
                ("step_hz", json!({ "type": "number", "exclusiveMinimum": 0, "description": "The spacing of the channels, Hz." })),
                ("offset_hz", number("A fixed offset added to every centre, Hz (ISDB-T: one seventh of a MHz).")),
                ("prefix", text("Text put before each number (\"E\", \"R\", \"PMR \").")),
                ("names", one_of_ids(&[naming(Naming::Number), naming(Naming::Icao833)], "How each channel is named: the prefix and the number, or the ICAO name of an 8.33 kHz channel.")),
            ],
        ),
    );
    defs.insert(
        "channel".into(),
        object(
            "One listed channel.",
            &["name", "centre_hz"],
            vec![
                ("name", text("Its name as the plan prints it (\"9A\", \"16\").")),
                ("centre_hz", hz("Its centre frequency, Hz.")),
                ("width_hz", json!({ "type": "number", "exclusiveMinimum": 0, "maximum": 100e6, "description": "Its width, Hz (default: the band's channel_width_hz)." })),
                ("pair_hz", hz("The other frequency of a duplex pair, Hz.")),
                ("note", text("A short note.")),
                ("in_use", json!({ "type": "boolean", "default": true, "description": "False for a channel kept out of use." })),
            ],
        ),
    );
    defs.insert(
        "raster".into(),
        object(
            "The tuning raster of a band without channels.",
            &["first_hz", "step_hz"],
            vec![
                ("first_hz", hz("The first point, Hz.")),
                ("step_hz", json!({ "type": "number", "exclusiveMinimum": 0, "description": "The spacing, Hz." })),
            ],
        ),
    );
    defs.insert(
        "cell_numbers".into(),
        object(
            "A range of downlink channel numbers: number n (first to last, in steps of step) is at first_hz + (n - first) * hz_per_number.",
            &["first", "last", "first_hz", "hz_per_number"],
            vec![
                ("first", json!({ "type": "integer", "minimum": 0, "description": "The first number." })),
                ("last", json!({ "type": "integer", "minimum": 0, "description": "The last number." })),
                ("step", json!({ "type": "integer", "minimum": 1, "default": 1, "description": "The numbers between two usable channels (NR: 20 for the 100 kHz raster)." })),
                ("first_hz", hz("The frequency of number first, Hz.")),
                ("hz_per_number", json!({ "type": "number", "exclusiveMinimum": 0, "description": "The frequency difference between two numbers, Hz." })),
                ("scs_khz", json!({ "type": "integer", "description": "The subcarrier spacing the range is for, kHz (NR bands with a 15 or 30 kHz channel raster)." })),
            ],
        ),
    );
    defs.insert(
        "sync_raster".into(),
        object(
            "The GSCN range of an NR band for one SSB subcarrier spacing.",
            &["scs_khz", "first", "last"],
            vec![
                ("scs_khz", json!({ "enum": [15, 30, 120, 240], "description": "The SSB subcarrier spacing, kHz." })),
                ("first", json!({ "type": "integer", "minimum": 0, "description": "The first GSCN." })),
                ("last", json!({ "type": "integer", "minimum": 0, "description": "The last GSCN." })),
                ("step", json!({ "type": "integer", "minimum": 1, "default": 1, "description": "The GSCN step." })),
            ],
        ),
    );
    defs.insert(
        "cellular".into(),
        object(
            "The channel numbers of a cellular band (3GPP). The band edges are the downlink edges.",
            &["system", "band", "duplex", "numbers"],
            vec![
                ("system", one_of_ids(&["gsm", "lte", "nr"], "The system: GSM (ARFCN), LTE (EARFCN) or NR (NR-ARFCN and GSCN).")),
                ("band", json!({ "type": "integer", "minimum": 0, "maximum": 65535, "description": "The band number (n78 is 78). GSM uses the band name (900, 1800)." })),
                ("duplex", one_of_ids(&["fdd", "tdd", "sdl"], "How the band uses its frequencies: FDD, TDD or supplementary downlink.")),
                ("numbers", list_of("cell_numbers", "The downlink channel numbers.")),
                ("sync", list_of("sync_raster", "The NR synchronisation raster, one range for each SSB spacing.")),
            ],
        ),
    );
    defs.insert(
        "band".into(),
        object(
            "One band of a group: its edges, the modes that receive it, and its channels or tuning raster.",
            &["name", "service", "start_hz", "end_hz"],
            vec![
                ("name", text("Its name (\"VHF Band III\", \"40 m\").")),
                ("service", one_of_ids(&services, "The radio service, after the ITU Radio Regulations Article 1.")),
                ("start_hz", hz("The lower edge, Hz.")),
                ("end_hz", hz("The upper edge, Hz.")),
                ("modes", mode_ids),
                ("in_use", json!({ "type": "boolean", "default": true, "description": "False when the band moved to another service." })),
                ("modulation", text("The usual modulation there, as a modulation id (\"am\", \"usb\", \"nfm\", \"wfm\").")),
                ("note", text("A note (required when in_use is false).")),
                ("channel_width_hz", json!({ "type": "number", "exclusiveMinimum": 0, "maximum": 100e6, "description": "The width of each channel, Hz." })),
                ("numbering", def_ref("numbering", "Numbered channels on a regular raster.")),
                ("channels", list_of("channel", "Channels listed one by one.")),
                ("raster", def_ref("raster", "The tuning raster of a band without channels.")),
                ("cellular", def_ref("cellular", "The channel numbers of a cellular band.")),
            ],
        ),
    );
    defs.insert(
        "group".into(),
        object(
            "A band group: a named set of bands from one published plan.",
            &["id", "name", "kind", "source", "verified"],
            vec![
                ("id", id.clone()),
                ("name", text("The name shown.")),
                ("kind", one_of_ids(&kinds, "What the group is for.")),
                ("source", def_ref("source", "The published document its values come from.")),
                ("also", list_of("source", "More documents that give some of its values.")),
                ("verified", json!({ "type": "boolean", "description": "Every value has been compared with the source." })),
                ("unverified_reason", text("Why it is not verified (required when verified is false).")),
                ("note", text("A note for the picker.")),
                (
                    "ended",
                    json!({
                        "type": "string",
                        "pattern": "^[0-9]{4}(-[0-9]{2}(-[0-9]{2})?)?$",
                        "description": "The date the service of this group ended (YYYY, YYYY-MM or YYYY-MM-DD). Absent while the service is live.",
                    }),
                ),
                ("analog", def_ref("analog", "The analogue TV system, for analogue TV groups.")),
                ("include", texts("Groups whose bands come first, before this group's own bands.")),
                ("bands", list_of("band", "The bands.")),
            ],
        ),
    );
    defs.insert(
        "presets".into(),
        object(
            "The region presets that no band group holds.",
            &[],
            vec![
                ("fm_deemphasis_us", json!({ "type": "number", "minimum": 0, "maximum": 1000, "description": "The FM pre-emphasis time constant, microseconds." })),
                ("rds", one_of_ids(&[rds(RdsVariant::Rds), rds(RdsVariant::Rbds)], "How FM receivers read RDS: RDS (IEC 62106) or RBDS (NRSC-4-B).")),
                ("tv_standard", json!({ "type": "string", "pattern": "^[a-z0-9_-]{1,64}$", "description": "The stack id of the digital TV standard to try first." })),
            ],
        ),
    );
    defs.insert(
        "country".into(),
        object(
            "One country: the groups it uses and its presets. For each kind, a country that lists no group of that kind uses its ITU region's groups, unless the kind is absent.",
            &["code"],
            vec![
                ("code", json!({ "type": "string", "pattern": "^[A-Z]{2}$", "description": "ISO 3166-1 alpha-2 code." })),
                ("name", text("English short name (required in the one entry of a set of files that names the country).")),
                ("itu", json!({ "type": "integer", "minimum": 1, "maximum": 3, "description": "ITU region (required in the one entry of a set of files that names the country)." })),
                ("groups", texts("The ids of the groups it uses, the default of each kind first.")),
                ("absent", json!({ "type": "array", "items": { "enum": kinds }, "description": "Plan kinds the country has none of." })),
                ("presets", def_ref("presets", "Its presets.")),
                ("verified", json!({ "type": "boolean", "default": false, "description": "The groups and presets have been compared with the sources." })),
                ("unverified_reason", text("Why the groups and presets are not verified.")),
                ("sources", list_of("source", "The documents that say which plans the country uses and its presets.")),
                ("note", text("A note.")),
            ],
        ),
    );
    defs.insert(
        "region".into(),
        object(
            "One ITU region: the groups and presets of a country that gives none of its own.",
            &["itu", "name"],
            vec![
                ("itu", json!({ "type": "integer", "minimum": 1, "maximum": 3, "description": "The ITU region." })),
                ("name", text("Its name.")),
                ("groups", texts("The ids of the groups used there, the default of each kind first.")),
                ("presets", def_ref("presets", "Its presets.")),
                ("note", text("A note.")),
            ],
        ),
    );
    let mut root = object(
        &format!(
            "A plan file of sdrkit, format version {FORMAT_VERSION}: band groups, countries and ITU regions. The plan files a \
             host loads and user files use this format. Generated from the model types of sdrkit-data-plans; do not edit."
        ),
        &["format"],
        vec![
            ("$schema", text("The schema the file follows, for editors.")),
            ("format", json!({ "const": FORMAT_VERSION, "description": "The format version." })),
            ("groups", list_of("group", "Band groups.")),
            ("countries", list_of("country", "Countries.")),
            ("regions", list_of("region", "ITU regions.")),
        ],
    );
    let r = root.as_object_mut().expect("an object");
    r.insert("$schema".into(), json!(DIALECT));
    r.insert("title".into(), json!("sdrkit band plans"));
    r.insert("$defs".into(), Value::Object(defs));
    root
}

/// The schema as the text checked into `docs/schema/plans.json` (indented, with a final line break).
pub fn json_schema_text() -> String {
    let mut s = serde_json::to_string_pretty(&json_schema()).unwrap_or_default();
    s.push('\n');
    s
}
