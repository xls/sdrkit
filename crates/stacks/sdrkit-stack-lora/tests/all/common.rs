//! Helpers of the test program: receivers, feeding, commands, and reading the declared tables.

use sdrkit_core::owner;
use sdrkit_core::publish::TableDef;
use sdrkit_core::settings::CatalogueSetting;
use sdrkit_core::verify::CaseSetting;
use sdrkit_core::{
    Complex32, Event, EventDef, InputInfo, Output, Receiver, Record, Results, StackDescriptor,
    Value,
};
use sdrkit_dev_test::Harness;
use sdrkit_stack_lora::mesh::types::Protocol;
use sdrkit_stack_lora::typed::{LoraPacketEvent, LORA_PACKET};

/// A receiver of stack `d` (sub-mode `sub`) at its defaults with these settings, given in the settings JSON form.
pub fn receiver(
    d: &'static StackDescriptor,
    sub: Option<&str>,
    settings: &[CaseSetting],
) -> Box<dyn Receiver> {
    let mut h = Harness::new(d);
    if let Some(s) = sub {
        h = h.submode(s);
    }
    h = h.with_settings(settings);
    h.receiver().expect("the receiver")
}

/// Runs the stack command `key` of `rx` with these arguments. The others take their defaults.
///
/// # Errors
///
/// The refusal of the command.
pub fn invoke(
    rx: &mut dyn Receiver,
    key: &str,
    args: &[(&str, Value)],
) -> Result<(), sdrkit_core::CommandError> {
    let id = rx.commands().id(key)?;
    let def = rx.commands().def(id).expect("the command").clone();
    let mut a = def.new_args();
    for (k, v) in args {
        let fid = a.decl().id(k).expect("an argument of the command");
        a.set_value(fid, v).expect("an argument of its kind");
    }
    owner::invoke(rx, id, &a).map(|_| ())
}

/// Feeds `x` (centred on `centre`, tuned to `tuned`) in pieces of `piece`, then ends the stream. Returns every event.
pub fn feed(
    rx: &mut dyn Receiver,
    x: &[Complex32],
    rate: f64,
    centre: f64,
    tuned: f64,
    piece: usize,
) -> Vec<Event> {
    rx.configure(&InputInfo::new(rate, centre, tuned).recording())
        .expect("configure");
    let mut out = Output::default();
    let mut events = Vec::new();
    for c in x.chunks(piece) {
        out.clear();
        rx.feed(c, &mut out);
        events.append(&mut out.events);
    }
    out.clear();
    rx.finish(&mut out);
    events.append(&mut out.events);
    events
}

/// The rows of table `def`, newest first.
pub fn rows<'a>(res: &'a Results, def: &TableDef) -> Vec<&'a Record> {
    let Some(t) = res.decl().table_id(def.key).and_then(|id| res.table(id)) else {
        return Vec::new();
    };
    t.rows_newest_first().map(|(_, r)| r).collect()
}

/// The newest row of table `def` for which `pred` holds.
pub fn row<'a>(
    res: &'a Results,
    def: &TableDef,
    pred: impl Fn(&Record) -> bool,
) -> Option<&'a Record> {
    rows(res, def).into_iter().find(|r| pred(r))
}

/// The rows of table `def` for which `pred` holds.
pub fn count(res: &Results, def: &TableDef, pred: impl Fn(&Record) -> bool) -> usize {
    rows(res, def).into_iter().filter(|r| pred(r)).count()
}

/// The events of one kind.
pub fn events<'a>(ev: &'a [Event], def: &EventDef) -> Vec<&'a Event> {
    ev.iter().filter(|e| e.is(def)).collect()
}

/// The `lora.packet` events of one protocol (every protocol for `None`).
pub fn packets(ev: &[Event], net: Option<Protocol>) -> Vec<&Event> {
    ev.iter()
        .filter(|e| e.is(&LORA_PACKET) && (net.is_none() || e.at(&LoraPacketEvent::NET) == net))
        .collect()
}

/// A generator run as the engine makes it: the radio tuned `tune_offset_hz` above the dial, the station at the dial. The signal
/// comes from `h` (its generator and impairments), and a fresh receiver of `h` takes it. Returns the events and the receiver.
pub fn gen_run(h: &Harness, d: &StackDescriptor, piece: usize) -> (Vec<Event>, Box<dyn Receiver>) {
    gen_run_at(h, d, dial(d), piece)
}

/// The starting dial of `d` that a host reads from `lora/default_dials.csv` through the helper library; 0 for a stack
/// without a row.
pub fn dial(d: &StackDescriptor) -> f64 {
    static DIALS: std::sync::OnceLock<Vec<sdrkit_presets::default_dials::DefaultDial>> =
        std::sync::OnceLock::new();
    let rows = DIALS.get_or_init(|| sdrkit_presets::radio_dials::dials(data()).expect("the dials"));
    sdrkit_presets::default_dials::dial_hz(rows, d.id, None).unwrap_or(0.0)
}

/// As [`gen_run`], with the station at `dial`.
pub fn gen_run_at(
    h: &Harness,
    d: &StackDescriptor,
    dial: f64,
    piece: usize,
) -> (Vec<Event>, Box<dyn Receiver>) {
    let x = h.generate().expect("the test signal");
    let mut rx = h.receiver().expect("the receiver");
    let ev = feed(
        rx.as_mut(),
        &x,
        d.tuning.rate_hz,
        dial + d.tuning.tune_offset_hz,
        dial,
        piece,
    );
    (ev, rx)
}

/// Entry `key` of catalogue setting `s`, as a test setting.
pub fn catalogue(s: &CatalogueSetting, key: &str) -> CaseSetting {
    let i = (s.choices)()
        .iter()
        .position(|c| c.key == key)
        .expect("an entry of the catalogue");
    CaseSetting {
        key: s.key,
        value: Value::Choice(u16::try_from(i).expect("a small catalogue")),
    }
}

/// The data folder of the repository, read by the helper library of hosts.
pub fn data() -> &'static sdrkit_presets::Data {
    static DATA: std::sync::OnceLock<sdrkit_presets::Data> = std::sync::OnceLock::new();
    DATA.get_or_init(|| {
        sdrkit_presets::Data::open(concat!(env!("CARGO_MANIFEST_DIR"), "/../../../data"))
            .expect("the data folder")
    })
}

/// The settings of a row of the data, as test settings.
pub fn settings_of(s: &sdrkit_presets::lora::LoraSettings) -> Vec<CaseSetting> {
    s.settings
        .iter()
        .map(|(k, v)| CaseSetting {
            key: Box::leak(k.clone().into_boxed_str()),
            value: v.clone(),
        })
        .collect()
}

/// The dial and the settings of the band plan `plan` of the data for the mesh stack `stack`.
pub fn band_plan(stack: &str, plan: &str) -> (f64, Vec<CaseSetting>) {
    let s = data()
        .mesh_band_plan_settings(plan, stack)
        .expect("a band plan of the stack");
    (s.frequency_hz.expect("a dial"), settings_of(&s))
}

/// The band of the Meshtastic region `region` of the data.
pub fn slot_band(region: &str) -> sdrkit_stack_lora::proto::meshtastic::SlotBand {
    let r = data()
        .meshtastic_regions()
        .expect("the regions")
        .into_iter()
        .find(|r| r.name == region)
        .expect("a region of the data");
    sdrkit_stack_lora::proto::meshtastic::SlotBand {
        start_hz: r.start_hz,
        end_hz: r.end_hz,
    }
}
