//! The page of one radio driver, written from its descriptor.

use crate::render::{
    code, commands_table, events_section, flags_section, hz, results_sections, setting_rows,
    settings_table, table, SETTING_HEADER,
};
use sdrkit_core::{Owner, SettingDecl, SettingSet};
use sdrkit_driver_core::iiod::tx::IiodTxSetting;
use sdrkit_driver_core::{DriverKind, RadioSetting, SourceDriver, StreamFormat, TxSetting};
use sdrkit_driver_soapyremote::SoapyTxSetting;

/// What a driver connects to, and an example address.
struct Facts {
    key: &'static str,
    connects: &'static str,
    example: &'static str,
}

const FACTS: &[Facts] = &[
    Facts {
        key: "synth",
        connects: "The test signal of a stack. No radio is involved.",
        example: "synth:fm",
    },
    Facts {
        key: "file",
        connects: "A recording in the storage folder: SigMF, WAV or raw IQ.",
        example: "file:recording.sigmf-meta",
    },
    Facts {
        key: "rtl_tcp",
        connects: "An rtl_tcp server, which shares an RTL-SDR receiver on the network.",
        example: "rtl_tcp:192.168.1.20:1234",
    },
    Facts {
        key: "iiod",
        connects: "A PlutoSDR, an ANTSDR or another AD936x board, through the iiod network daemon.",
        example: "iiod:192.168.2.1",
    },
    Facts {
        key: "spyserver",
        connects: "A SpyServer, which shares one radio over TCP.",
        example: "spyserver:192.168.1.30:5555",
    },
    Facts {
        key: "sdrpp",
        connects: "The server mode of SDR++, which streams the baseband of one radio.",
        example: "sdrpp:192.168.1.30",
    },
    Facts {
        key: "soapyremote",
        connects: "A SoapyRemote server, which shares the SoapySDR radios of one computer.",
        example: "soapyremote:192.168.1.30",
    },
    Facts {
        key: "kiwisdr",
        connects:
            "A KiwiSDR receiver on the network. The driver reads the IQ stream of one channel.",
        example: "kiwisdr:kiwi.example.org:8073",
    },
    Facts {
        key: "openwebrx",
        connects: "An OpenWebRX server. The server sends demodulated audio and never IQ.",
        example: "openwebrx:sdr.example.org:8073",
    },
    Facts {
        key: "rsp_tcp",
        connects: "An rsp_tcp server, which shares an SDRplay RSP receiver on the network.",
        example: "rsp_tcp:192.168.1.20",
    },
    Facts {
        key: "sdrconnect",
        connects: "SDRconnect, the receiver program of SDRplay, through its WebSocket interface.",
        example: "sdrconnect:192.168.1.30",
    },
    Facts {
        key: "tci",
        connects: "A program that serves the TCI interface, such as ExpertSDR3 for SunSDR radios.",
        example: "tci:192.168.1.30",
    },
    Facts {
        key: "hermeslite2",
        connects: "A Hermes Lite 2 radio, through the HPSDR protocol 1.",
        example: "hermeslite2:192.168.1.40:1024",
    },
    Facts {
        key: "redpitaya",
        connects: "A Red Pitaya radio, through the HPSDR protocol 1.",
        example: "redpitaya:192.168.1.41:1024",
    },
    Facts {
        key: "hpsdr1",
        connects: "A radio of the HPSDR protocol 1 (Metis, Hermes, Angelia, Orion).",
        example: "hpsdr1:192.168.1.42:1024",
    },
    Facts {
        key: "hpsdr2",
        connects: "A radio of the HPSDR protocol 2 (ANAN, Orion firmware).",
        example: "hpsdr2:192.168.1.43:1024",
    },
    Facts {
        key: "rtlusb",
        connects: "An RTL2832U dongle (RTL-SDR) on a USB port of this computer.",
        example: "usb:rtl:",
    },
    Facts {
        key: "hackrf",
        connects: "A HackRF One, HackRF Pro, Jawbreaker or rad1o on a USB port of this computer. It transmits.",
        example: "usb:hackrf:",
    },
];

/// The keys of the drivers that have facts. The test makes sure that this list and the registry agree.
pub fn keys() -> Vec<&'static str> {
    FACTS.iter().map(|f| f.key).collect()
}

fn facts(key: &str) -> &'static Facts {
    FACTS
        .iter()
        .find(|f| f.key == key)
        .unwrap_or_else(|| panic!("the driver {key} has no entry in the list of address forms"))
}

/// The one-line description of a driver for the index.
pub fn connects(key: &str) -> &'static str {
    facts(key).connects
}

/// The settings that the driver adds to the settings of every radio.
fn split(settings: &[SettingDecl]) -> (&[SettingDecl], &[SettingDecl]) {
    settings.split_at(RadioSetting::SETTINGS.len().min(settings.len()))
}

/// The settings of a transmitter of the driver, as declared: an open radio narrows them to its limits. The driver's own
/// transmit settings follow the common four.
pub fn transmit_settings(key: &str) -> Vec<SettingDecl> {
    let own: &[&dyn sdrkit_core::DeclareSetting] = match key {
        "iiod" => IiodTxSetting::SETTINGS,
        "soapyremote" => SoapyTxSetting::SETTINGS,
        "hackrf" => sdrkit_driver_hackrf::HackrfTxSetting::SETTINGS,
        _ => &[],
    };
    let all: Vec<&dyn sdrkit_core::DeclareSetting> =
        TxSetting::SETTINGS.iter().chain(own).copied().collect();
    sdrkit_core::Schema::of(&all).settings().to_vec()
}

fn streams(title: &str, list: &[StreamFormat]) -> String {
    let rows: Vec<Vec<String>> = list
        .iter()
        .map(|f| {
            vec![
                format!("{:?}", f.kind),
                format!("{:?}", f.sample),
                format!("{} to {}", hz(f.min_rate_hz), hz(f.max_rate_hz)),
                f.channels.to_string(),
            ]
        })
        .collect();
    format!(
        "{title}\n\n{}\n",
        table(&["Kind", "Sample", "Rates", "Channels"], &rows)
    )
}

/// The page of the driver.
pub fn page(driver: Box<dyn SourceDriver>) -> String {
    let d = driver.descriptor();
    let f = facts(d.key);
    let decl = d.decl();
    let kind = DriverKind::new(driver);
    let mut out = format!("# {}\n\n{}\n\n", d.name, f.connects);
    out.push_str(&format!("Driver key: {}.\n\n", code(d.key)));

    out.push_str("## Address\n\n");
    for a in d.addresses {
        let (form, note) = a.split_once(" (").unwrap_or((a, ""));
        match note.strip_suffix(')') {
            Some(note) => out.push_str(&format!("- {}: {note}.\n", code(form))),
            None => out.push_str(&format!("- {}\n", code(form))),
        }
    }
    out.push('\n');
    out.push_str(match d.opens {
        sdrkit_driver_core::Opens::Once => {
            "The device has one receiver: one radio of an address is open at a time, and the sessions share it.\n\n"
        }
        sdrkit_driver_core::Opens::Many => "Each open radio of an address is a receiver of its own.\n\n",
    });

    let (radio, own) = split(decl.schema().settings());
    out.push_str(&settings_section(
        "Radio settings",
        "Every radio has these. Give one as `--set key=value`. If the stack has a setting with the same key, write `radio.key=value`.",
        radio,
    ));
    out.push_str(&settings_section(
        "Driver settings",
        "Only this driver has these.",
        own,
    ));
    out.push_str("The radio can add settings when it opens, such as the gain of each stage. `show radio` lists them on the console of `run`.\n\n");
    if d.key == "synth" {
        out.push_str(&signal_section());
    }

    if !d.produces.is_empty() {
        out.push_str(&streams("## Streams it produces", d.produces));
    }

    out.push_str(&results_sections(decl.results(), "Readings"));
    out.push_str(&events_section(decl.results().events()));
    out.push_str(&flags_section(decl.results()));

    out.push_str("## Commands of an open radio\n\n");
    out.push_str(&commands_table(decl.commands().defs()));
    out.push('\n');

    let kind_commands = kind.commands();
    if !kind_commands.is_empty() {
        out.push_str("## Commands of the driver\n\nThey work before any radio is open. Run one with `sdrkit call kind:driver:");
        out.push_str(d.key);
        out.push_str(" <command>`.\n\n");
        out.push_str(&commands_table(kind_commands.defs()));
        out.push('\n');
    }
    let kind_results = results_sections(kind.results_decl(), "Driver readings");
    if !kind_results.is_empty() {
        out.push_str(&kind_results);
    }

    if !d.accepts.is_empty() {
        out.push_str("## Transmit\n\nThe radio can transmit. The range of the transmit power and the power that the transmitter starts with come from the radio when it opens: the bottom of the range for a radio that can drive an amplifier, 10 dB below full power for a low-power board.\n\n");
        out.push_str(&settings_table(&transmit_settings(d.key)));
        out.push('\n');
        out.push_str(&streams("Streams it accepts:", d.accepts));
    }

    out.push_str(&format!(
        "## Try it\n\n```sh\nsdrkit run fm --source {}\n```\n",
        f.example
    ));
    out
}

/// The settings that the engine adds to a radio of the test signal of a stack.
fn signal_section() -> String {
    let at = sdrkit_core::DeclareSetting::decl(&sdrkit_host_engine::GEN_AT_HZ);
    format!(
        "## Settings of the test signal\n\nWhen the radio opens, it adds the settings of the stack whose test signal it plays and this setting. Give them as `--set key=value`.\n\n{}\n",
        table(&SETTING_HEADER, &setting_rows(&[at]))
    )
}

fn settings_section(title: &str, lead: &str, settings: &[SettingDecl]) -> String {
    let mut out = format!("## {title}\n\n");
    if settings.is_empty() {
        out.push_str("The driver adds none.\n\n");
        return out;
    }
    out.push_str(&format!("{lead}\n\n"));
    out.push_str(&table(&SETTING_HEADER, &setting_rows(settings)));
    out.push('\n');
    out
}

/// The keys of the setting tables of a page: the radio settings and the driver settings together, in the order of the schema.
pub fn schema_keys(driver: &dyn SourceDriver) -> Vec<String> {
    driver
        .descriptor()
        .decl()
        .schema()
        .settings()
        .iter()
        .map(|s| s.field.key.to_string())
        .collect()
}
