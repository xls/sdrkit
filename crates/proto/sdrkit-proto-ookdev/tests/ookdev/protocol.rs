//! The protocol list: modulation filter, despike, de-duplication and the per-train context.

use crate::common::train;
use sdrkit_proto_ookdev::reading::measure;
use sdrkit_proto_ookdev::{
    BitRows, Check, Coding, Ctx, Error, Modulation, Protocol, ProtocolDef, ProtocolList, Reading,
    Timing,
};

/// 8 PWM bits per row. Each row of 8 bits produces a reading, and the id is the byte.
fn per_row(rows: &BitRows, _: &mut Ctx, out: &mut Vec<Reading>) {
    for row in rows.iter().filter(|r| r.len == 8) {
        out.push(Reading::new(
            "Row",
            "sensor",
            row.byte(0).to_string(),
            Check::Repeat,
        ));
    }
}

/// The same, but a code counts only on its second copy in the train.
fn second_copy(rows: &BitRows, ctx: &mut Ctx, out: &mut Vec<Reading>) {
    for row in rows.iter().filter(|r| r.len == 8) {
        if !ctx.first_copy(row.byte(0) as u64) {
            out.push(Reading::new(
                "Twice",
                "remote",
                row.byte(0).to_string(),
                Check::Repeat,
            ));
        }
    }
}

const PWM: Timing = Timing::new(Coding::Pwm, 400.0, 1200.0, 5000.0).with_gap(2000.0);

static ROW: ProtocolDef = ProtocolDef {
    name: "Row",
    modulation: Modulation::Ook,
    timing: PWM,
    despike_us: 0.0,
    decode: per_row,
};
static TWICE: ProtocolDef = ProtocolDef {
    name: "Twice",
    modulation: Modulation::Ook,
    timing: PWM,
    despike_us: 0.0,
    decode: second_copy,
};
static FSK_ROW: ProtocolDef = ProtocolDef {
    name: "FSK row",
    modulation: Modulation::Fsk,
    timing: PWM,
    despike_us: 0.0,
    decode: per_row,
};
static DESPIKED: ProtocolDef = ProtocolDef {
    name: "Despiked",
    modulation: Modulation::Ook,
    timing: PWM,
    despike_us: 20.0,
    decode: per_row,
};

fn byte_pairs(bytes: &[u8]) -> Vec<(f32, f32)> {
    let mut p = Vec::new();
    for &b in bytes {
        for k in 0..8 {
            p.push(if (b >> (7 - k)) & 1 == 1 {
                (400.0, 1200.0)
            } else {
                (1200.0, 400.0)
            });
        }
        p.last_mut().unwrap().1 = 3000.0;
    }
    p
}

#[test]
fn list_offers_by_modulation_and_keeps_one_of_each() {
    let mut list = ProtocolList::new();
    list.add_static(&ROW);
    list.add_static(&FSK_ROW);
    list.add_static(&TWICE);
    assert_eq!(list.len(), 3);
    assert_eq!(list.get(2).map(|p| p.name()), Some("Twice"));
    let tr = train(Modulation::Ook, 1e6, &byte_pairs(&[0x3C, 0x3C, 0x81]));
    let mut out = Vec::new();
    let n = list.decode(&tr, &mut out).unwrap();
    // "Row" decodes 3C (twice, kept once) and 81. "FSK row" is not offered. "Twice" decodes 3C on its second copy.
    let got: Vec<(&str, &str, usize)> = out
        .iter()
        .map(|r| (&*r.model, &*r.id, r.protocol))
        .collect();
    assert_eq!(n, 3);
    assert_eq!(
        got,
        vec![("Row", "60", 0), ("Row", "129", 0), ("Twice", "60", 2)]
    );
}

#[test]
fn despike_runs_before_slicing() {
    // Each mark is split by a 5 µs dropout.
    let mut p = Vec::new();
    for (m, s) in byte_pairs(&[0xA5]) {
        let h = (m - 5.0) / 2.0;
        p.push((h, 5.0));
        p.push((h, s));
    }
    let tr = train(Modulation::Ook, 1e6, &p);
    let mut plain = ProtocolList::new();
    plain.add_static(&ROW);
    let mut out = Vec::new();
    assert_eq!(plain.decode(&tr, &mut out).unwrap(), 0);
    let mut list = ProtocolList::new();
    list.add_static(&DESPIKED);
    assert_eq!(list.decode(&tr, &mut out).unwrap(), 1);
    assert_eq!(out[0].id, "165");
}

#[test]
fn train_measurements_reach_the_readings() {
    let mut list = ProtocolList::new();
    list.add_table(std::slice::from_ref(&ROW));
    let mut tr = train(Modulation::Ook, 1e6, &byte_pairs(&[0x42]));
    tr.start = 2.5e6;
    tr.offset_hz = 12e3;
    tr.level_dbfs = -30.0;
    tr.snr_db = 18.0;
    let mut out = Vec::new();
    list.decode(&tr, &mut out).unwrap();
    let r = &out[0];
    assert_eq!(
        (r.time_s, r.offset_hz, r.level_dbfs, r.snr_db),
        (2.5, 12e3, -30.0, 18.0)
    );
}

#[test]
fn a_protocol_type_of_its_own() {
    struct Mine(Timing);
    impl Protocol for Mine {
        fn name(&self) -> &str {
            "Mine"
        }
        fn modulation(&self) -> Modulation {
            Modulation::Ook
        }
        fn timing(&self) -> &Timing {
            &self.0
        }
        fn decode(&self, rows: &BitRows, ctx: &mut Ctx, out: &mut Vec<Reading>) {
            per_row(rows, ctx, out)
        }
    }
    let mut list = ProtocolList::new();
    assert_eq!(list.add(Mine(PWM)), 0);
    let mut out = Vec::new();
    let tr = train(Modulation::Ook, 1e6, &byte_pairs(&[7]));
    assert_eq!(list.decode(&tr, &mut out).unwrap(), 1);
}

#[test]
fn bad_trains_are_refused() {
    let mut list = ProtocolList::new();
    list.add_static(&ROW);
    let mut tr = train(Modulation::Ook, 1e6, &byte_pairs(&[7]));
    tr.pulses[3].space = -1.0;
    assert_eq!(list.decode(&tr, &mut Vec::new()), Err(Error::BadTrain));
}

#[test]
fn context_remembers_codes_per_protocol() {
    let mut c = Ctx::default();
    assert!(c.first_copy(5));
    assert!(!c.first_copy(5));
    for k in 0..200 {
        c.first_copy(k + 100);
    }
    c.clear();
    assert!(c.first_copy(5));
}

#[test]
fn readings_and_fields() {
    let r = Reading::new("Model", "weather", "12", Check::Crc)
        .with_channel_name("B")
        .num(&measure::TEMPERATURE_C, -3.25, 1)
        .with(&measure::DOOR_OPEN, true)
        .with(&measure::BUTTON, "A");
    assert_eq!(r.line(), "Temperature -3.2 °C, Door open yes, Button A");
    assert_eq!(r.channel.to_string(), "B");
    let j = serde_json::to_string(&r).unwrap();
    let back: Reading = serde_json::from_str(&j).unwrap();
    assert!(back.same_as(&r));
    assert_eq!(back.get(&measure::BUTTON), Some("A"));
    let unknown = j.replace("door_open", "door_shut");
    assert!(serde_json::from_str::<Reading>(&unknown).is_err());
}

#[test]
fn a_reading_leaves_the_graph_as_an_event() {
    use sdrkit_core::interface::ReadFields;
    use sdrkit_core::publish::keys;
    use sdrkit_graph::Item;
    use sdrkit_proto_ookdev::reading::{ReadingFields, READING_EVENT};
    let r = Reading::new("Model", "weather", "12", Check::Crc)
        .with_channel(2)
        .num(&measure::TEMPERATURE_C, 21.5, 1)
        .with(&measure::SEQUENCE, 7);
    let mut e = READING_EVENT.event(1.0, 433.92e6);
    r.write_fields(&mut e.fields);
    assert_eq!(e.get(&keys::SOURCE), Some("Model 12"));
    assert_eq!(e.get(&keys::TEXT), Some("Temperature 21.5 °C, Sequence 7"));
    assert_eq!(e.get(&keys::CHANNEL), Some("2"));
    assert_eq!(e.at(&ReadingFields::CHECK), Some(Check::Crc));
    assert_eq!(e.at(measure::TEMPERATURE_C.column()), Some(21.5));
    assert_eq!(e.at(measure::SEQUENCE.column()), Some(7));
    assert_eq!(e.at(measure::HUMIDITY_PCT.column()), None);
    let f = ReadingFields::read(&e.fields).expect("the fields of a reading");
    assert_eq!((f.model, f.device_kind, f.id), ("Model", "weather", "12"));
    assert_eq!(
        (f.temperature_c, f.sequence, f.humidity_pct),
        (Some(21.5), Some(7), None)
    );
    assert_eq!(f.battery_ok, None);
    assert_eq!(measure::ALL[0].key(), "temperature_c");
    assert!(measure::ALL.iter().all(|f| f.key() != "snr_db"));
}

#[test]
fn meter_and_climate_readings_keep_their_units() {
    let inhg = 29.92;
    let btu_h = 1000.0;
    let r = Reading::new("Meter", "meter", "7", Check::Crc)
        .num(&measure::VOLTAGE_V, 230.1, 1)
        .num(&measure::ENERGY_KWH, 1234.5, 1)
        .num(&measure::GAS_VOLUME_M3, 12.25, 2)
        .num(&measure::WATER_VOLUME_M3, 3.5, 1)
        .num(&measure::LUMINANCE_PCT, 40.0, 0)
        .num(&measure::ABSOLUTE_HUMIDITY_G_M3, 8.5, 1)
        .num(&measure::PRESSURE_KPA, inhg * measure::KPA_PER_INHG, 1)
        .num(&measure::POWER_W, btu_h * measure::W_PER_BTU_IT_H, 0);
    let j = serde_json::to_string(&r).unwrap();
    let back: Reading = serde_json::from_str(&j).unwrap();
    assert!(back.same_as(&r));
    assert!(r.line().contains("Pressure 101.3 kPa"), "{}", r.line());
    assert!(r.line().contains("Power 293 W"), "{}", r.line());
    assert!(r.line().contains("Gas volume 12.25 m³"), "{}", r.line());
    assert!(
        r.line().contains("Absolute humidity 8.5 g/m³"),
        "{}",
        r.line()
    );
}

/// The shared interface lists the result sets of the three components and the reading event, whose shape has the
/// fields of its declaration.
#[test]
fn the_shared_interface_lists_the_components_and_the_reading() {
    let shared = sdrkit_proto_ookdev::components::interface();
    assert!(shared.shared);
    let stack = &shared.stacks[0];
    let kinds: Vec<&str> = stack.events.iter().map(|e| e.def.kind).collect();
    assert_eq!(kinds, ["ookdev.reading"]);
    assert_eq!(stack.events[0].def.fields.len(), 12 + measure::ALL.len());
    let keys: Vec<String> = stack
        .results_decl()
        .fields()
        .fields()
        .iter()
        .map(|f| f.key.to_string())
        .collect();
    assert_eq!(
        keys,
        [
            "pulses.trains",
            "burst_pulses.trains",
            "burst_pulses.dropped",
            "decode.readings",
            "decode.unknown_trains"
        ]
    );
}
