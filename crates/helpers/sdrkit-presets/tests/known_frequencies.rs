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
];

/// The gaps that are still to fill: the stack and the countries of the table where it has no known frequency.
const OPEN: &[(&str, &str)] = &[
    ("stl", "AU DE GB JP"),
    ("dvb_terrestrial", "AU US DE GB JP"),
    ("dvbsh", "AU US DE GB JP"),
    ("meteor_lrpt", "AU US DE GB JP"),
    ("goes_hrit", "AU DE GB JP"),
    ("gk2a_lrit", "US DE GB"),
    ("gk2a_hrit", "US DE GB"),
    ("elektro_l", "AU US DE GB JP"),
    ("fy4_lrit", "AU US DE GB JP"),
    ("fy4a_hrit", "AU US DE GB JP"),
    ("fy2_svissr", "AU US DE GB JP"),
    ("fy3_ahrpt", "AU US DE GB JP"),
    ("fy3_mpt", "AU US DE GB JP"),
    ("noaa_gac", "AU US DE GB JP"),
    ("msg_raw", "AU US DE GB JP"),
    ("goes_cda", "AU US DE GB JP"),
    ("goes_mdl", "AU US DE GB JP"),
    ("gk2a_cdas", "AU US DE GB JP"),
    ("sband_ccsds", "AU US DE GB JP"),
    ("mesh", "JP"),
    ("meshtastic", "JP"),
    ("meshcore", "JP"),
    ("loraaprs", "AU US JP"),
    ("meshcom", "AU US JP"),
    ("lorawanwide", "AU US DE GB JP"),
    ("inmarsatc", "AU US DE GB JP"),
    ("globalstar", "AU US DE GB JP"),
    ("sarsat", "AU US DE GB JP"),
    ("goes_dcs", "AU US DE GB JP"),
    ("hrdcp", "AU US DE GB JP"),
    ("argos", "AU US DE GB JP"),
    ("cubesat", "AU US DE GB JP"),
    ("fleet_broadcast", "AU US DE GB JP"),
    ("lora_sat", "AU US DE GB JP"),
    ("qo100", "AU US DE GB JP"),
    ("bgan", "AU US DE GB JP"),
    ("fsk441", "AU US DE GB JP"),
    ("jt6m", "AU US DE GB JP"),
    ("iscat", "AU US DE GB JP"),
    ("pi4", "AU US DE GB JP"),
    ("isdbtsb", "AU US DE GB JP"),
    ("mpt1327", "AU US DE GB JP"),
    ("ltr", "AU US DE GB JP"),
    ("edacs", "AU US DE GB JP"),
    ("tones", "AU US DE GB JP"),
    ("pager", "AU US DE GB JP"),
    ("multitone", "AU US DE GB JP"),
    ("ermes", "AU US JP"),
    ("motorola_type2", "AU US DE GB JP"),
    ("opensky", "AU US DE GB JP"),
    ("gsc", "AU US DE GB JP"),
    ("scada", "AU US DE GB JP"),
    ("selcal", "AU US DE GB JP"),
    ("airband", "AU US DE GB JP"),
    ("hfdl", "AU US DE GB JP"),
    ("vdl3", "AU US DE GB JP"),
    ("gbas_vdb", "AU US DE GB JP"),
    ("ldacs", "AU US DE GB JP"),
    ("aeromacs", "AU US DE GB JP"),
    ("vdes", "AU US DE GB JP"),
    ("marinevhf", "AU US DE GB JP"),
    ("atis", "AU US DE GB JP"),
    ("dvbc2", "AU US DE GB JP"),
    ("dvbs", "AU US DE GB JP"),
    ("isdbs", "AU US DE GB JP"),
    ("satbeacon", "AU US DE GB JP"),
    ("ebicab", "AU US DE GB JP"),
    ("eot", "AU US DE GB JP"),
    ("umts", "AU US DE GB JP"),
    ("cdma2000", "AU US DE GB JP"),
    ("tdscdma", "AU US DE GB JP"),
    ("cv2x", "AU US DE GB JP"),
    ("railway_cir", "AU US DE GB JP"),
    ("mac", "AU US DE GB JP"),
    ("fpv", "AU US DE GB JP"),
    ("ism", "US"),
    ("wmbus", "AU US JP"),
    ("zwave", "AU JP"),
    ("sigfox", "AU US DE GB JP"),
    ("subghzchat", "JP"),
    ("dect", "JP"),
    ("nfc", "AU US DE GB JP"),
    ("wisun", "AU US DE GB JP"),
    ("mioty", "AU US DE GB JP"),
    ("dect_nrplus", "AU US DE GB JP"),
    ("npr70", "AU US DE GB JP"),
    ("enocean", "AU"),
    ("sitor", "AU US DE GB JP"),
    ("throb", "AU US DE GB JP"),
    ("qrss", "AU US DE GB JP"),
    ("cp16", "AU US DE GB JP"),
    ("winmor", "AU US DE GB JP"),
    ("sbas", "AU US DE GB JP"),
    ("navic", "AU US DE GB JP"),
    ("remote_id", "AU US DE GB JP"),
    ("flarm", "AU US DE GB JP"),
    ("adsl", "AU US JP"),
    ("ogn", "AU US DE GB JP"),
    ("rc_links", "AU US DE GB JP"),
    ("mavlink_sik", "AU US DE GB JP"),
    ("dme", "AU US DE GB JP"),
    ("loran", "AU US DE GB JP"),
    ("ale", "AU US DE GB JP"),
    ("pactor1", "AU US DE GB JP"),
    ("wavebuoy", "AU US DE GB JP"),
    ("hfradar", "AU US DE GB JP"),
    ("hfbeacons", "AU US DE GB JP"),
    ("pactor2", "AU US DE GB JP"),
    ("pactor3", "AU US DE GB JP"),
    ("pactor4", "AU US DE GB JP"),
    ("ms110", "AU US DE GB JP"),
    ("s4285", "AU US DE GB JP"),
    ("s4539", "AU US DE GB JP"),
    ("ale3g", "AU US DE GB JP"),
    ("s4481", "AU US DE GB JP"),
    ("link11", "AU US DE GB JP"),
    ("efr", "AU US DE GB JP"),
    ("gtor", "AU US DE GB JP"),
    ("arqe", "AU US DE GB JP"),
    ("autospec", "AU US DE GB JP"),
    ("piccolo", "AU US DE GB JP"),
    ("coquelet", "AU US DE GB JP"),
    ("cis_hf", "AU US DE GB JP"),
    ("crowd36", "AU US DE GB JP"),
    ("xpa", "AU US DE GB JP"),
    ("fsk200", "AU US DE GB JP"),
    ("gwfsk", "AU US DE GB JP"),
    ("fsk_modems", "AU US DE GB JP"),
    ("uvb76", "AU US DE GB JP"),
    ("vlf_msk", "AU US DE GB JP"),
    ("halow", "AU US DE GB JP"),
    ("wfb", "AU US DE GB JP"),
    ("sonde", "AU US DE GB JP"),
    ("eczas", "AU US DE GB JP"),
    ("ssdv", "AU US DE GB JP"),
    ("astro", "AU US DE GB JP"),
    ("iono", "AU US DE GB JP"),
    ("supersid", "AU US DE GB JP"),
    ("jove", "AU US DE GB JP"),
    ("callisto", "AU US DE GB JP"),
    ("riometer", "AU US DE GB JP"),
    ("eme_echo", "AU US DE GB JP"),
    ("beacon_events", "AU US DE GB JP"),
    ("pulsar", "AU US DE GB JP"),
    ("radar_detect", "AU US DE GB JP"),
    ("horus", "AU US DE GB JP"),
    ("elt121", "AU US DE GB JP"),
    ("v2x", "AU US DE GB JP"),
    ("rfid_lf", "AU US DE GB JP"),
    ("rfid_uhf", "AU US DE GB JP"),
    ("qi", "AU US DE GB JP"),
];

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
        let unused = NOT_USED
            .iter()
            .any(|(s, c, _)| *s == r.stack && holds(c, r.country));
        let open = OPEN
            .iter()
            .any(|(s, c)| *s == r.stack && holds(c, r.country));
        if gap && !free && !unused && !open {
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
