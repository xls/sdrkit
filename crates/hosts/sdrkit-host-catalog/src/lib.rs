//! The registry of the stacks and the component kinds of sdrkit: one line per stack crate, and the stack families that
//! have an interface description. Each stack crate is a cargo feature of this crate, all on by default: a host that
//! needs fewer families builds with `default-features = false` and names the ones it wants.

#![warn(missing_docs)]

use sdrkit_core::codec::CodecSet;
use sdrkit_core::interface::Family;
use sdrkit_core::{Facilities, StackDescriptor};
use sdrkit_graph::Registry;

/// Every stack with the key of its family, in the order of the stack list. The family of a stack is the stack crate
/// that holds it: `sdrkit-stack-<family>`.
pub fn stacks_by_family() -> Vec<(&'static str, &'static StackDescriptor)> {
    fn add(
        out: &mut Vec<(&'static str, &'static StackDescriptor)>,
        family: &'static str,
        stacks: impl IntoIterator<Item = &'static StackDescriptor>,
    ) {
        out.extend(stacks.into_iter().map(|s| (family, s)));
    }
    // The DVB-T or DVB-T2 stack takes its DVB-T2 receiver from the DVB-T2 crate. The first call sets it, later calls change
    // nothing.
    #[cfg(feature = "dvbt")]
    sdrkit_stack_dvbt::terrestrial::set_t2_factory(sdrkit_stack_dvbt2::make_receiver);
    let mut stacks = Vec::new();
    #[cfg(feature = "fm")]
    add(&mut stacks, "fm", sdrkit_stack_fm::descriptors()); // FM broadcast with RDS, STL links
    #[cfg(feature = "general")]
    add(&mut stacks, "general", sdrkit_stack_general::descriptors()); // AM, NFM, WFM, SSB, CW and the listening chain
    #[cfg(feature = "adsb")]
    add(&mut stacks, "adsb", sdrkit_stack_adsb::descriptors()); // ADS-B, UAT, Mode A/C, 1030 MHz interrogations
    #[cfg(feature = "dab")]
    add(&mut stacks, "dab", [&sdrkit_stack_dab::DESCRIPTOR]); // DAB and DAB+ digital radio
    #[cfg(feature = "dvbt")]
    add(&mut stacks, "dvbt", sdrkit_stack_dvbt::descriptors()); // DVB-T, DVB-T or DVB-T2 on one frequency, DVB-SH
    #[cfg(feature = "dvbt2")]
    add(&mut stacks, "dvbt2", sdrkit_stack_dvbt2::descriptors()); // DVB-T2
    #[cfg(feature = "dvoice")]
    add(&mut stacks, "dvoice", sdrkit_stack_dvoice::descriptors()); // DMR, NXDN, dPMR, TETRA, P25, TETRAPOL, D-STAR, YSF, M17, FreeDV, DCR
    #[cfg(feature = "satimg")]
    add(&mut stacks, "satimg", sdrkit_stack_satimg::descriptors()); // Meteor, HRPT, FY-3, FY-2, GOES, GK-2A, FY-4, Elektro-L
    #[cfg(feature = "lora")]
    add(&mut stacks, "lora", sdrkit_stack_lora::descriptors()); // LoRa, LoRaWAN, Meshtastic, MeshCore, MeshCom, LoRa APRS, APRS
    #[cfg(feature = "satcom")]
    add(&mut stacks, "satcom", sdrkit_stack_satcom::descriptors()); // Inmarsat, BGAN, Iridium, Orbcomm, Globalstar, SARSAT, the carrier survey and others
    #[cfg(feature = "wsjt")]
    add(&mut stacks, "wsjt", sdrkit_stack_wsjt::descriptors()); // FT8, FT4, FT2, WSPR, JT65, JT9, JT4, Q65, FST4, FST4W, MSK144, JS8
    #[cfg(feature = "tvterr")]
    add(&mut stacks, "tvterr", sdrkit_stack_tvterr::descriptors()); // ISDB-T, ISDB-TSB, ATSC, ATSC 3.0, DTMB, TV auto
    #[cfg(feature = "broadcast")]
    add(
        &mut stacks,
        "broadcast",
        sdrkit_stack_broadcast::descriptors(),
    ); // DRM, HamDRM, HD Radio, CDR, C-QUAM
    #[cfg(feature = "trunk")]
    add(&mut stacks, "trunk", sdrkit_stack_trunk::descriptors()); // MPT1327, LTR, EDACS, tones, pagers, Multitone
    #[cfg(feature = "avdata")]
    add(&mut stacks, "avdata", sdrkit_stack_avdata::descriptors()); // ACARS, VDL2, VDL4, HFDL, airband, SELCAL
    #[cfg(feature = "marine")]
    add(&mut stacks, "marine", sdrkit_stack_marine::descriptors()); // AIS, DSC, NAVTEX, weather fax, VDES, marine VHF, ATIS
    #[cfg(feature = "dvbcs")]
    add(&mut stacks, "dvbcs", sdrkit_stack_dvbcs::descriptors()); // DVB-C, DVB-C2, DVB-S, DVB-S2, ISDB-S, satellite beacon
    #[cfg(feature = "cell")]
    add(&mut stacks, "cell", sdrkit_stack_cell::descriptors()); // LTE, 5G NR, NB-IoT, GSM-R, LBJ, EBICAB, EOT, UMTS, CDMA2000, TD-SCDMA, C-V2X
    #[cfg(feature = "analogtv")]
    add(
        &mut stacks,
        "analogtv",
        sdrkit_stack_analogtv::descriptors(),
    ); // PAL, SECAM, NTSC with NICAM and teletext, D2-MAC, FPV
    #[cfg(feature = "ism")]
    add(&mut stacks, "ism", sdrkit_stack_ism::descriptors()); // ISM devices, wM-Bus, Z-Wave, Zigbee, Wi-SUN, Sigfox, ANT and others
    #[cfg(feature = "hfkbd")]
    add(&mut stacks, "hfkbd", sdrkit_stack_hfkbd::descriptors()); // RTTY, PSK, MFSK, Olivia, DominoEX, Hell, SSTV, SITOR, ARDOP, CW, WINMOR
    #[cfg(feature = "gnss")]
    add(&mut stacks, "gnss", sdrkit_stack_gnss::descriptors()); // GPS, Galileo, GLONASS, BeiDou, SBAS
    #[cfg(feature = "drones")]
    add(&mut stacks, "drones", sdrkit_stack_drones::descriptors()); // Remote ID, DJI DroneID, FLARM, ADS-L, OGN, RC links, MAVLink on SiK
    #[cfg(feature = "avnav")]
    add(&mut stacks, "avnav", sdrkit_stack_avnav::descriptors()); // NDB, VOR, ILS, DME, Loran-C, DGPS
    #[cfg(feature = "hfdata")]
    add(&mut stacks, "hfdata", sdrkit_stack_hfdata::descriptors()); // ALE, PACTOR, HF identification, beacons, HF radar, wave buoys, HF teleprinters, CIS, the HF senders (XPA, CROWD36, FSK200, GW FSK, telephone FSK modems, UVB-76, VLF MSK)
    #[cfg(feature = "wlan")]
    add(&mut stacks, "wlan", sdrkit_stack_wlan::descriptors()); // BLE, BLE trackers, Wi-Fi, ESP-NOW, Wi-Fi HaLow, Bluetooth Classic, wfb-ng / OpenHD
    #[cfg(feature = "science")]
    add(&mut stacks, "science", sdrkit_stack_science::descriptors()); // Radiosondes, time signals, e-Czas, SAME, meteor scatter, SSDV, astronomy
    #[cfg(feature = "voice")]
    add(&mut stacks, "voice", sdrkit_stack_voice::descriptors()); // HF packet, ELT 121.5 MHz, voice inversion
    #[cfg(feature = "road")]
    add(&mut stacks, "road", sdrkit_stack_road::descriptors()); // 802.11p V2X
    #[cfg(feature = "custom")]
    add(&mut stacks, "custom", sdrkit_stack_custom::descriptors()); // Slotted digital voice, QAM video link, picture in a burst, packet link, file transfer
    #[cfg(feature = "meter")]
    add(&mut stacks, "meter", sdrkit_stack_meter::descriptors()); // modulation monitors, spectrum watchers, transmitter tests, interference and instrument health
    #[cfg(feature = "tags")]
    add(&mut stacks, "tags", sdrkit_stack_tags::descriptors()); // EM4100 LF tags, EPC Gen2 reader commands, Qi charging
    stacks
}

/// Every stack, in the order of the stack list. Ids are unique.
pub fn all() -> Vec<&'static StackDescriptor> {
    stacks_by_family().into_iter().map(|(_, s)| s).collect()
}

/// The key of the family of the stack `id`. `None` if the catalog does not have the stack.
pub fn family_key_of(id: &str) -> Option<&'static str> {
    stacks_by_family()
        .into_iter()
        .find(|(_, s)| s.id == id)
        .map(|(family, _)| family)
}

/// Adds the interface description of each named stack crate that is built and has one. The build script sets
/// `typed_<family>` for a crate with `src/typed.rs`, whose `typed::family()` is the description.
macro_rules! described {
    ($families:ident: $($feature:literal $typed:ident $krate:ident,)*) => {
        $(
            #[cfg(all(feature = $feature, $typed))]
            $families.push($krate::typed::family());
        )*
    };
}

/// The stack families that have an interface description, in the order of the stack list. The C library, the registry
/// check and the pages of the stacks take the families from here. A stack crate is in the list from the moment it has
/// `src/typed.rs` with `pub fn family() -> &'static Family`.
#[allow(unused_mut)]
pub fn families() -> Vec<&'static Family> {
    let mut families: Vec<&'static Family> = Vec::new();
    described!(families:
        "fm" typed_fm sdrkit_stack_fm,
        "general" typed_general sdrkit_stack_general,
        "adsb" typed_adsb sdrkit_stack_adsb,
        "dab" typed_dab sdrkit_stack_dab,
        "dvbt" typed_dvbt sdrkit_stack_dvbt,
        "dvbt2" typed_dvbt2 sdrkit_stack_dvbt2,
        "dvoice" typed_dvoice sdrkit_stack_dvoice,
        "satimg" typed_satimg sdrkit_stack_satimg,
        "lora" typed_lora sdrkit_stack_lora,
        "satcom" typed_satcom sdrkit_stack_satcom,
        "wsjt" typed_wsjt sdrkit_stack_wsjt,
        "tvterr" typed_tvterr sdrkit_stack_tvterr,
        "broadcast" typed_broadcast sdrkit_stack_broadcast,
        "trunk" typed_trunk sdrkit_stack_trunk,
        "avdata" typed_avdata sdrkit_stack_avdata,
        "marine" typed_marine sdrkit_stack_marine,
        "dvbcs" typed_dvbcs sdrkit_stack_dvbcs,
        "cell" typed_cell sdrkit_stack_cell,
        "analogtv" typed_analogtv sdrkit_stack_analogtv,
        "ism" typed_ism sdrkit_stack_ism,
        "hfkbd" typed_hfkbd sdrkit_stack_hfkbd,
        "gnss" typed_gnss sdrkit_stack_gnss,
        "drones" typed_drones sdrkit_stack_drones,
        "avnav" typed_avnav sdrkit_stack_avnav,
        "hfdata" typed_hfdata sdrkit_stack_hfdata,
        "wlan" typed_wlan sdrkit_stack_wlan,
        "science" typed_science sdrkit_stack_science,
        "voice" typed_voice sdrkit_stack_voice,
        "road" typed_road sdrkit_stack_road,
        "custom" typed_custom sdrkit_stack_custom,
        "meter" typed_meter sdrkit_stack_meter,
        "tags" typed_tags sdrkit_stack_tags,
    );
    families
}

/// The interface description of the family of the stack `id`. `None` if the stack is in no family with a description.
pub fn family_of(id: &str) -> Option<&'static Family> {
    let key = family_key_of(id)?;
    families()
        .into_iter()
        .find(|f| f.key == key && f.stack_shape(id).is_some())
}

/// The codec plug-ins of the workspace: the AudioToolbox decoders on Apple platforms, then the decoders of
/// `sdrkit-codec-aac` (AAC-LC, HE-AAC and HE-AAC v2) and of `sdrkit-codec-mpa` (Layer II), which take the place of
/// AudioToolbox for those codecs, and the half-rate and full-rate speech decoders of the digital voice stacks.
pub fn codecs() -> CodecSet {
    let mut set = CodecSet::default();
    #[cfg(any(target_os = "macos", target_os = "ios"))]
    sdrkit_codec_apple::register(&mut set);
    sdrkit_codec_aac::register(&mut set);
    sdrkit_codec_mpa::register(&mut set);
    #[cfg(feature = "dvoice")]
    sdrkit_stack_dvoice::mbe::register(&mut set);
    set
}

/// The component kinds: the standard ones, those of every stack crate, the channel bank's and the recorders', so a graph
/// description can use any of them. The recorders write through the storage of `facilities` and name files by its clock.
pub fn registry(facilities: &Facilities) -> Registry {
    let mut r = sdrkit_components::registry();
    #[cfg(feature = "fm")]
    sdrkit_stack_fm::graph::register(&mut r);
    #[cfg(feature = "general")]
    sdrkit_stack_general::register(&mut r);
    #[cfg(feature = "adsb")]
    sdrkit_stack_adsb::register(&mut r);
    #[cfg(feature = "dvbt")]
    sdrkit_stack_dvbt::register(&mut r); // DVB-T and the transport stream components
    #[cfg(feature = "satimg")]
    sdrkit_stack_satimg::register(&mut r);
    #[cfg(feature = "lora")]
    sdrkit_stack_lora::register(&mut r);
    #[cfg(feature = "satcom")]
    sdrkit_stack_satcom::register(&mut r);
    #[cfg(feature = "dab")]
    sdrkit_stack_dab::register(&mut r);
    #[cfg(feature = "tvterr")]
    sdrkit_stack_tvterr::register(&mut r);
    #[cfg(feature = "dvbt2")]
    sdrkit_stack_dvbt2::register(&mut r);
    #[cfg(feature = "avdata")]
    sdrkit_stack_avdata::register(&mut r);
    #[cfg(feature = "marine")]
    sdrkit_stack_marine::register(&mut r);
    #[cfg(feature = "dvbcs")]
    sdrkit_stack_dvbcs::register(&mut r);
    #[cfg(feature = "analogtv")]
    sdrkit_stack_analogtv::register(&mut r);
    #[cfg(feature = "hfkbd")]
    sdrkit_stack_hfkbd::register(&mut r);
    #[cfg(feature = "gnss")]
    sdrkit_stack_gnss::register(&mut r);
    #[cfg(feature = "avnav")]
    sdrkit_stack_avnav::register(&mut r);
    #[cfg(feature = "hfdata")]
    sdrkit_stack_hfdata::register(&mut r);
    #[cfg(feature = "wlan")]
    sdrkit_stack_wlan::register(&mut r);
    #[cfg(feature = "science")]
    sdrkit_stack_science::register(&mut r);
    #[cfg(feature = "voice")]
    sdrkit_stack_voice::register(&mut r);
    #[cfg(feature = "meter")]
    sdrkit_stack_meter::register(&mut r);
    #[cfg(feature = "tags")]
    sdrkit_stack_tags::register(&mut r);
    sdrkit_svc_bank::register(&mut r); // the channel bank's components
    sdrkit_svc_record::register(&mut r, facilities.storage.clone(), facilities.clock.clone()); // the rewind, audio and transport stream recorders
    r
}

/// An engine with every stack, component kind, codec and driver of the workspace, on the network and the clock of the
/// system. It keeps its files in memory: nothing is written to a disk. Make the engine with [`sdrkit_host_engine::Engine::new`]
/// for another storage.
///
/// # Errors
///
/// What [`sdrkit_host_engine::Engine::new`] returns.
pub fn engine() -> Result<sdrkit_host_engine::Engine, sdrkit_host_engine::EngineError> {
    engine_on(std::sync::Arc::new(sdrkit_core::net::StdNet))
}

/// As [`engine`], on the network `transport`.
///
/// # Errors
///
/// What [`sdrkit_host_engine::Engine::new`] returns.
pub fn engine_on(
    transport: std::sync::Arc<dyn sdrkit_core::net::Transport>,
) -> Result<sdrkit_host_engine::Engine, sdrkit_host_engine::EngineError> {
    let facilities = Facilities {
        storage: std::sync::Arc::new(sdrkit_core::MemStorage::new()),
        clock: sdrkit_host_engine::system_clock(),
        random: sdrkit_host_engine::system_random(),
        codecs: std::sync::Arc::new(codecs()),
    };
    sdrkit_host_engine::Engine::new(sdrkit_host_engine::Setup {
        components: registry(&facilities),
        facilities,
        transport,
        stacks: all(),
        families: families(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use sdrkit_core::{StackConfig, Values};
    use std::sync::Arc;

    #[test]
    fn stack_ids_are_unique_and_every_stack_is_made_from_its_defaults() {
        let stacks = all();
        let mut ids: Vec<&str> = stacks.iter().map(|s| s.id).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), stacks.len());
        let registry = registry(&Facilities::default());
        let adapters: Arc<dyn sdrkit_core::AdapterCatalog> = registry.catalog().clone();
        for s in stacks {
            assert!(!s.blurb.is_empty() && !s.group.is_empty(), "{}", s.id);
            assert!(s.tuning.min_rate_hz <= s.tuning.rate_hz, "{}", s.id);
            let cfg =
                StackConfig::new(Values::defaults(&(s.settings)())).with_adapters(adapters.clone());
            assert!((s.make_receiver)(&cfg).is_ok(), "{}", s.id);
        }
    }

    /// What a stack transmits is declared once, on its commands. The receiver of each stack with such a command has a
    /// transmitter for the engine to connect.
    #[test]
    fn a_stack_with_a_command_that_transmits_can_transmit() {
        let registry = registry(&Facilities::default());
        let adapters: Arc<dyn sdrkit_core::AdapterCatalog> = registry.catalog().clone();
        let mut transmitting = 0;
        for s in all() {
            let Some(shape) = family_of(s.id).and_then(|f| f.stack_shape(s.id)) else {
                continue;
            };
            if shape.commands.iter().all(|c| c.command.transmits.is_none()) {
                continue;
            }
            transmitting += 1;
            let cfg =
                StackConfig::new(Values::defaults(&(s.settings)())).with_adapters(adapters.clone());
            let rx = (s.make_receiver)(&cfg).expect("the receiver is made from its defaults");
            assert!(
                rx.transmits(),
                "{} has a command that transmits but no transmitter",
                s.id
            );
        }
        assert!(transmitting >= 10, "{transmitting} stacks transmit");
    }

    #[test]
    fn the_registry_has_the_component_kinds_of_the_stacks() {
        let r = registry(&Facilities::default());
        let names: Vec<&str> = r.names().collect();
        for n in [
            "fm.demod",
            "general.demod",
            "adsb.demod",
            "dvbt.sync",
            "dab.ofdm",
            "atsc.fec",
            "dsp.spectrum",
        ] {
            assert!(names.contains(&n), "{n}");
        }
        let wrong: Vec<&str> = names
            .iter()
            .copied()
            .filter(|n| r.make(n).map(|c| c.info().name != *n).unwrap_or(true))
            .collect();
        assert!(wrong.is_empty(), "{wrong:?}");
    }

    /// The most channels of a host audio input (`host.in`) in the graph of `rx`. 0 for a receiver without a graph.
    fn graph_host_audio(rx: &mut dyn sdrkit_core::Receiver) -> u16 {
        use sdrkit_graph::components::HostIn;
        let Some(g) = rx
            .as_any_mut()
            .and_then(|a| a.downcast_mut::<sdrkit_graph::Graph>())
        else {
            return 0;
        };
        g.order()
            .into_iter()
            .filter_map(|id| g.component_as::<HostIn>(id))
            .map(HostIn::channels)
            .max()
            .unwrap_or(0)
    }

    /// The host audio channels that a stack declares are those that its graph takes: the most over its default and
    /// its submodes, each made with the defaults. A submode that cannot be made counts 0.
    #[test]
    fn the_declared_host_audio_channels_are_those_of_the_graph() {
        let facilities = Facilities {
            codecs: Arc::new(codecs()),
            ..Facilities::default()
        };
        let registry = registry(&facilities);
        let adapters: Arc<dyn sdrkit_core::AdapterCatalog> = registry.catalog().clone();
        let mut wrong = Vec::new();
        for s in all() {
            let cfg =
                StackConfig::new(Values::defaults(&(s.settings)())).with_adapters(adapters.clone());
            let submodes = s.submodes.iter().map(|m| cfg.clone().with_submode(m.id));
            let built = std::iter::once(cfg.clone())
                .chain(submodes)
                .filter_map(|c| {
                    let mut rx = (s.make_receiver)(&c).ok()?;
                    rx.set_facilities(&facilities);
                    Some(graph_host_audio(rx.as_mut()))
                })
                .max()
                .unwrap_or(0);
            if built != s.tuning.host_audio_channels {
                wrong.push((s.id, s.tuning.host_audio_channels, built));
            }
        }
        assert!(
            wrong.is_empty(),
            "declared and built host audio channels differ: {wrong:?}"
        );
    }
}
