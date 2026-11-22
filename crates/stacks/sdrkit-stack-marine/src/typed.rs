//! The interface of the marine family: what each of its five stacks declares ([`family`]). The shapes, tables and event
//! kinds of a stack are in the module of the stack.

use crate::ais::display as ais;
use crate::ais::generator::AisGenSetting;
use crate::ais::receiver::{CHANNELS_SETTING, COLLISION_CANCEL};
use crate::{atis, marine, vdes, vhf};
use sdrkit_core::interface::{Family, StackShape};
use sdrkit_core::publish::SeriesDef;
use sdrkit_core::SettingSet;
use sdrkit_util_objects::commands::CLEAR;
use std::sync::OnceLock;

/// The interface of the family.
pub fn family() -> &'static Family {
    static FAMILY: OnceLock<Family> = OnceLock::new();
    FAMILY.get_or_init(describe)
}

fn describe() -> Family {
    // Version 1.2 names rows and orders tables.
    Family::new("marine", 1, 3)
        .added(
            1,
            1,
            &[
                "ais.content",
                "marine.content",
                "vdes.content",
                "marinevhf.content",
                "atis.content",
            ],
        )
        .content(sdrkit_core::ContentKind::Voice, &["marinevhf", "atis"])
        .content(sdrkit_core::ContentKind::Tracking, &["ais", "vdes"])
        .content(sdrkit_core::ContentKind::Datalink, &["marine"])
        .audio(sdrkit_core::AudioKind::Voice, &["marinevhf", "atis"])
        .audio(sdrkit_core::AudioKind::Monitor, &["marine"])
        .text(&["marine"])
        .stack(
            StackShape::new("ais")
                .fields::<ais::AisFields>()
                .table(&ais::STATION_TABLE)
                .table(&ais::TRACK_POINTS)
                .table(&ais::COUNTERS)
                .table(&ais::CHANNEL_TABLE)
                .table(&ais::SENTENCES)
                .table(&ais::RECEPTIONS)
                .table(&ais::ALERTS)
                .table(&ais::HOP_PLAN)
                .table(&ais::SELECTED)
                .points("Stations")
                .event(&ais::VESSEL)
                .event(&ais::NMEA)
                .event(&ais::CPA_ALERT)
                .event(&ais::GEOFENCE_ALERT)
                .settings(&[&CHANNELS_SETTING, &COLLISION_CANCEL])
                .settings(&ais::StationsSetting::SETTINGS[1..])
                .settings(AisGenSetting::SETTINGS)
                .command(&ais::SELECT)
                .command(&CLEAR)
                .command(&ais::REMOVE),
        )
        .stack(
            StackShape::new("marine")
                .fields::<marine::HfFields>()
                .table(&marine::NAVTEX_TABLE)
                .table(&marine::DSC_TABLE)
                .series(SeriesDef {
                    key: marine::SPECTRUM_SERIES,
                    label: "Audio spectrum",
                    x_unit: "Hz",
                    y_unit: "dB",
                })
                .points("DSC positions")
                .event(&marine::NAVTEX_MESSAGE)
                .event(&marine::DSC_CALL)
                .settings(marine::OWN)
                .settings(crate::hfgen::MarineGenSetting::SETTINGS)
                .command(&CLEAR)
                .command(&marine::CLEAR_PICTURE),
        )
        .stack(
            StackShape::new("vdes")
                .fields::<vdes::VdesFields>()
                .table(&vdes::ASM_TABLE)
                .table(&vdes::BURST_TABLE)
                .table(&vdes::CHANNEL_TABLE)
                .table(&vdes::BY_MODULATION)
                .table(&vdes::BY_BANDWIDTH)
                .points("ASM positions")
                .event(&vdes::ASM_FRAME)
                .event(&vdes::ASM_SENTENCE)
                .event(&vdes::TER_BURST)
                .settings(vdes::VdesSetting::SETTINGS)
                .command(&CLEAR),
        )
        .stack(
            StackShape::new("marinevhf")
                .fields::<vhf::VhfFields>()
                .table(&vhf::CALL_TABLE)
                .table(&vhf::CHANNEL_TABLE)
                .event(&vhf::CALL)
                .settings(vhf::VhfSetting::SETTINGS)
                .command(&CLEAR)
                .command(&vhf::CLOSE_ALL),
        )
        .stack(
            StackShape::new("atis")
                .fields::<atis::AtisFields>()
                .table(&atis::MESSAGE_TABLE)
                .table(&atis::STATION_TABLE)
                .event(&atis::MESSAGE)
                .settings(atis::AtisSetting::SETTINGS)
                .command(&CLEAR)
                .command(&atis::REMOVE),
        )
}

/// The interface description of stack `id` of the family.
///
/// # Panics
///
/// If the family has no stack `id`.
pub fn shape(id: &str) -> &'static StackShape {
    match family().stack_shape(id) {
        Some(s) => s,
        None => panic!("the family has no stack {id}"),
    }
}
