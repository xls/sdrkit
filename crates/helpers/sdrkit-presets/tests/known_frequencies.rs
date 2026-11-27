//! The known frequencies that the data folder of the repository gives each stack of the catalog in some countries: the
//! channels (`Data::stack_channels`), the dials (`Data::dials`), the entries of the frequency database
//! (`Data::frequency_rows`) and the start of the preset (`Data::preset`).
//!
//! `cargo test -p sdrkit-presets --test known_frequencies -- --nocapture` prints the table.
//!
//! The gate: a stack has at least one known frequency in each country of the table where its system is used. A stack
//! of [`NO_FIXED_FREQUENCY`] has none by its nature; a country of [`NOT_USED`] does not use the system. [`OPEN`] lists
//! the gaps that are still to fill; each of them must still be a gap, so that the list only shrinks.

use sdrkit_presets::{Data, Options, Region};

/// The countries of the table.
const COUNTRIES: &[&str] = &["AU", "US", "DE", "GB", "JP"];

/// The stack families and the stacks that have no fixed frequency, each with the reason. A family key is the stack
/// crate `sdrkit-stack-<family>`.
const NO_FIXED_FREQUENCY: &[(&str, &str)] = &[
    ("dvbs", "satellite transponders belong to an orbital position, not to a country; the host lists those of the satellite the dish points at"),
    ("satbeacon", "satellite beacons belong to an orbital position, not to a country; the host lists those of the satellite the dish points at"),
    ("bgan", "the BGAN forward carriers belong to the satellite and change with the load; the stack finds them in the L band"),
    ("multitone", "on-site and hospital paging systems are licensed to one site each; no public channel list exists, and the stack measures whatever carrier the user tunes"),
    ("tones", "selective-call tones and codes on whatever voice channel the user tunes"),
    ("iono", "chirp sounders sweep the whole HF range; the stack listens wherever the user tunes"),
    ("meter", "a measurement of whatever signal the user tunes"),
    (
        "custom",
        "links whose frequency the user chooses; there is no published allocation",
    ),
    (
        "tv-auto",
        "it finds the standard of the channel that another stack or the user tuned",
    ),
    ("psk_demod", "a blind analyser of any narrow signal"),
    (
        "sat_survey",
        "it measures satellite carriers without a public specification wherever the user tunes",
    ),
    (
        "hfid",
        "it classifies any HF data signal that the user tunes",
    ),
    (
        "schumann",
        "it reads an ELF sensor on a sound card; there is no radio frequency",
    ),
    (
        "whistler",
        "it reads VLF audio from a sound card; there is no radio frequency",
    ),
    (
        "sferics",
        "it counts lightning impulses in VLF audio from a sound card; there is no radio frequency",
    ),
    (
        "lightning_tdoa",
        "it times sferics in VLF audio from a sound card; there is no radio frequency",
    ),
];

/// The countries of the table where a system is not used, each with the reason.
const NOT_USED: &[(&str, &str, &str)] = &[
    (
        "dab",
        "US",
        "plans/countries.json lists DAB as absent in the United States",
    ),
    (
        "dvbt",
        "US JP",
        "the TV standard of the country in plans/countries.json is ATSC or ISDB-T",
    ),
    (
        "dvbc",
        "AU JP",
        "the countries of plans/countries.json list cable TV as absent",
    ),
    (
        "atsc",
        "AU DE GB JP",
        "the TV standard of the country in plans/countries.json is DVB-T or ISDB-T",
    ),
    (
        "atsc3",
        "AU DE GB JP",
        "the TV standard of the country in plans/countries.json is DVB-T or ISDB-T",
    ),
    (
        "isdbt",
        "AU US DE GB",
        "the TV standard of the country in plans/countries.json is DVB-T or ATSC",
    ),
    (
        "hdradio",
        "AU DE GB JP",
        "IBOC digital radio is the system of the United States (47 CFR 73.404)",
    ),
    (
        "uat",
        "AU DE GB JP",
        "978 MHz UAT is a system of the United States (14 CFR 91.225)",
    ),
    (
        "dtmb",
        "AU US DE GB JP",
        "DTMB is the TV system of China, Hong Kong, Macau, Cuba and Pakistan",
    ),
    (
        "cdr",
        "AU US DE GB JP",
        "CDR is the digital radio system of China",
    ),
    ("mpt1327", "US JP", "MPT 1327 is the trunking standard of the United Kingdom and the countries that followed it; it is not deployed in the United States or Japan"),
    ("opensky", "AU DE GB JP", "OpenSky is a United States trunking product; it is not deployed in these countries"),
    ("dvb_terrestrial", "US JP", "the TV standard of the country in plans/countries.json is ATSC or ISDB-T"),
    ("isdbtsb", "AU US DE GB", "ISDB-T and its one-segment service are the TV system of Japan and South America"),
    ("dvbc2", "AU JP", "the countries of plans/countries.json list cable TV as absent"),
    ("dvbsh", "AU US GB JP", "the 2 GHz mobile-satellite operators of Decision 626/2008/EC serve the EU"),
    ("isdbs", "AU US DE GB", "ISDB-S is the satellite TV system of Japan"),
    ("mac", "AU US DE GB JP", "no MAC (D2-MAC, B-MAC) service is known on air; not confirmed with a regulator"),
    ("vdl3", "AU US DE GB JP", "VDL Mode 3 was not brought into service; no ground stations are known"),
    ("ldacs", "AU US DE GB JP", "LDACS is in trials; no service on air is known"),
    ("goes_hrit", "AU DE GB JP", "the GOES satellites serve the Americas; the country is outside their footprint"),
    ("gk2a_lrit", "US DE GB", "GK-2A at 128.2 E serves east Asia and Australia; the country is outside its footprint"),
    ("gk2a_hrit", "US DE GB", "GK-2A at 128.2 E serves east Asia and Australia; the country is outside its footprint"),
    ("fy2_svissr", "US DE GB", "the FY-2 satellites serve Asia; the country is outside their footprint"),
    ("fy4_lrit", "US DE GB", "the FY-4 satellites serve Asia and the western Pacific; the country is outside their footprint"),
    ("fy4a_hrit", "US DE GB", "the FY-4 satellites serve Asia and the western Pacific; the country is outside their footprint"),
    ("msg_raw", "AU US JP", "Meteosat at 0 degrees serves Europe and Africa; the country is outside its footprint"),
    ("elektro_l", "US", "Elektro-L near 76 E serves Europe, Africa and Asia; the country is outside its footprint"),
    ("adsl", "AU US JP", "ADS-L SRD 860 is the EASA link of Europe"),
    ("wmbus", "AU US JP", "Wireless M-Bus (EN 13757-4) is the metering radio of Europe"),
    ("railway_cir", "AU US DE GB JP", "the CIR data channels are those of the railways of China"),
    ("eot", "AU DE GB JP", "the end-of-train links of the AAR standard serve North America"),
    ("umts", "AU US DE GB JP", "the operators of the country have closed their 3G networks (announcements of 2021 to 2026)"),
    ("cdma2000", "AU US DE GB JP", "no CDMA2000 network is on air in the country (closed by 2022, or never built)"),
    ("tdscdma", "AU US DE GB JP", "TD-SCDMA was a network of China only, now closed"),
    ("eczas", "AU US JP", "e-Czas is the long-wave time service of Poland, heard in Europe"),
    ("enocean", "AU", "not licensed: 868.3 and 902.875 MHz lie in no band of the ACMA LIPD class licence 2025; 928.35 MHz lies in Table 4 item 7 (928 to 935 MHz), but no EnOcean product for Australia is known"),
];

/// The gaps that are still to fill: the stack and the countries of the table where it has no known frequency.
const OPEN: &[(&str, &str)] = &[];

fn data() -> Data {
    Data::open(concat!(env!("CARGO_MANIFEST_DIR"), "/../../../data")).expect("the data folder")
}

/// What the data gives one stack in one country.
#[derive(Debug)]
struct Row {
    family: &'static str,
    stack: &'static str,
    blurb: &'static str,
    country: &'static str,
    channels: usize,
    dials: usize,
    entries: usize,
    /// The first and the last channel, MHz.
    span: Option<(f64, f64)>,
    /// The start of the preset, MHz, and its plan; or why there is none.
    start: Result<(f64, String), String>,
}

fn rows(d: &Data) -> Vec<Row> {
    let mut out = Vec::new();
    for (family, s) in sdrkit_host_catalog::stacks_by_family() {
        for &country in COUNTRIES {
            let region = Region::new(country).expect("a country code");
            let channels = d.stack_channels(s.id, &region).expect("channels");
            let dials = d.dials(s.id, &region, None).expect("dials");
            let entries = d.frequency_rows(s.id, &region).expect("entries");
            let start = d
                .preset(s.id, &region, &Options::default())
                .map(|p| (p.frequency_hz / 1e6, p.plan))
                .map_err(|e| e.to_string());
            out.push(Row {
                family,
                stack: s.id,
                blurb: s.blurb,
                country,
                channels: channels.len(),
                dials: dials.len(),
                entries: entries.len(),
                span: channels
                    .first()
                    .zip(channels.last())
                    .map(|(a, b)| (a.frequency_hz / 1e6, b.frequency_hz / 1e6)),
                start,
            });
        }
    }
    out
}

#[test]
fn the_table_of_known_frequencies() {
    let d = data();
    let rows = rows(&d);
    println!(
        "family\tstack\tcountry\tchannels\tdials\tentries\tfirst_mhz\tlast_mhz\tstart_mhz\tplan"
    );
    for r in &rows {
        let (first, last) = r.span.map_or((String::new(), String::new()), |(a, b)| {
            (a.to_string(), b.to_string())
        });
        let (start, plan) = match &r.start {
            Ok((hz, plan)) => (hz.to_string(), plan.clone()),
            Err(e) => (String::from("-"), e.clone()),
        };
        println!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{first}\t{last}\t{start}\t{plan}",
            r.family, r.stack, r.country, r.channels, r.dials, r.entries
        );
    }
    assert!(!rows.is_empty());
}

/// True if `list` (codes apart by spaces) holds `code`.
fn holds(list: &str, code: &str) -> bool {
    list.split_whitespace().any(|c| c == code)
}

#[test]
fn every_stack_has_a_known_frequency_where_its_system_is_used() {
    let d = data();
    let rows = rows(&d);
    let ids: Vec<&str> = rows.iter().flat_map(|r| [r.stack, r.family]).collect();
    for (id, _) in NO_FIXED_FREQUENCY {
        assert!(
            ids.contains(id),
            "NO_FIXED_FREQUENCY names {id}, which is neither a stack nor a family"
        );
    }
    let mut problems = Vec::new();
    for r in &rows {
        let gap = r.channels + r.dials + r.entries == 0;
        let free = NO_FIXED_FREQUENCY
            .iter()
            .any(|(id, _)| *id == r.stack || *id == r.family);
        // A mode that the data marks out of service in the country has no channels there.
        let ended = Region::new(r.country).is_ok_and(|c| d.service_end(r.stack, &c).is_some());
        let unused = NOT_USED
            .iter()
            .any(|(s, c, _)| *s == r.stack && holds(c, r.country));
        let open = OPEN
            .iter()
            .any(|(s, c)| *s == r.stack && holds(c, r.country));
        if gap && !free && !unused && !open && !ended {
            problems.push(format!(
                "{} {}: no known frequency ({})",
                r.stack, r.country, r.blurb
            ));
        }
        if !gap && open {
            problems.push(format!(
                "{} {}: filled; take it out of OPEN",
                r.stack, r.country
            ));
        }
        if !gap && unused {
            problems.push(format!(
                "{} {}: has known frequencies, but NOT_USED names it",
                r.stack, r.country
            ));
        }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

/// The cell stacks: in each country of the table, the channels come from a plan of the country's own (a group named after
/// the stack and listed for the country), at least one of them is a band of the country, the start of the preset lies
/// inside a band of the country, and the preset sets the band order of the country (`cell/band_order.csv`). A starting
/// dial of another country, alone or as the start, fails.
#[test]
fn cell_stacks_offer_the_bands_of_the_country() {
    let d = data();
    let mut problems = Vec::new();
    for stack in ["cell", "nr", "nbiot"] {
        for &country in COUNTRIES {
            let region = Region::new(country).expect("a country code");
            let channels = d.stack_channels(stack, &region).expect("channels");
            let bands: Vec<(f64, f64)> = channels
                .iter()
                .filter_map(|c| c.width_hz.filter(|w| *w >= 5e6).map(|w| (c.frequency_hz - w / 2.0, c.frequency_hz + w / 2.0)))
                .collect();
            if bands.is_empty() {
                problems.push(format!("{stack} {country}: no band of the country among {} channels", channels.len()));
                continue;
            }
            match d.preset(stack, &region, &Options::default()) {
                Ok(p) => {
                    if !p.plan.starts_with(&format!("{stack}_")) {
                        problems.push(format!("{stack} {country}: the start comes from {}, not from a plan of the country", p.plan));
                    }
                    // The band order of the country, so that a cell is named by a band of the country.
                    let order = p.settings.iter().find(|(k, _)| k == "band_order");
                    if !matches!(order, Some((_, sdrkit_core::Value::Text(t))) if !t.is_empty()) {
                        problems.push(format!("{stack} {country}: no band order of the country: {:?}", p.settings));
                    }
                    if !bands.iter().any(|(lo, hi)| (*lo..=*hi).contains(&p.frequency_hz)) {
                        problems.push(format!("{stack} {country}: the start {} MHz lies in no band of the country", p.frequency_hz / 1e6));
                    }
                }
                Err(e) => problems.push(format!("{stack} {country}: no start: {e}")),
            }
        }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

/// A mode that a country entry lists as out of service has no channels there, and the helper says why; the same mode is
/// in service elsewhere.
#[test]
fn a_mode_out_of_service_has_no_channels_and_a_reason() {
    let d = data();
    let au = Region::new("AU").expect("a country code");
    let why = d.service_end("umts", &au).expect("umts is out of service in AU");
    assert_eq!(why.ended, "2024");
    assert!(d.stack_channels("umts", &au).expect("channels").is_empty());
    let (_, sentence) = d.service_end_why("umts", "3G UMTS cells", &au).expect("a sentence");
    assert_eq!(sentence, "3G (UMTS) was switched off in Australia in 2024");
    let us = Region::new("US").expect("a country code");
    assert!(d.service_end("umts", &us).is_none());
    assert!(d.service_end_why("umts", "3G UMTS cells", &us).is_none());
}

/// Public GSM ended in Australia, and the railway GSM-R there lies outside the bands of the GSM stack: the stack has no
/// channels in AU, and the sentence says so. Elsewhere it keeps its bands.
#[test]
fn gsm_is_out_of_service_in_australia() {
    let d = data();
    let au = Region::new("AU").expect("a country code");
    let (end, sentence) = d.service_end_why("gsmr", "2G GSM / GSM-R", &au).expect("GSM ended in AU");
    assert_eq!((end.year(), end.remains), (Some(2018), None));
    assert_eq!(sentence, "2G (GSM) was switched off in Australia in 2018");
    assert!(d.stack_channels("gsmr", &au).expect("channels").is_empty());
    let de = Region::new("DE").expect("a country code");
    assert!(d.service_end("gsmr", &de).is_none());
}
