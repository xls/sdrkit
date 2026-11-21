//! The interface descriptions of the objects that every host has: the session, the radio and the engine. The typed
//! functions of the C header, the Swift files and the registry records of these objects are written from them, as
//! those of a stack family are.
//!
//! A shape here is made from the declarations that the owner itself uses: the list of its result fields, its event
//! types and its commands. The description adds what a declaration does not say: the name of the struct, and which
//! fields can be without a value.

use crate::engine_owner::{self as eo, EngineSetting};
use crate::session::{SessionSetting, OWN_FIELDS};
use sdrkit_core::interface::{
    ArgShape, CommandShape, EventShape, Family, FieldNote, Shape, StackShape, TableShape,
};
use sdrkit_core::{Command, Declare, EventDef, SettingSet, TableDef};
use sdrkit_driver_core::RadioSetting;
use std::sync::OnceLock;

/// A text of the description. The descriptions are made once and live as long as the program.
fn keep(text: String) -> &'static str {
    Box::leak(text.into_boxed_str())
}

/// The shape `name` of `fields`. The fields with a key in `optional` can be without a value. `docs` says what a field
/// holds, by key. A field without an entry has its label.
fn shape(
    name: &'static str,
    doc: &'static str,
    fields: &'static [&'static dyn Declare],
    optional: &[&str],
    docs: &[(&str, &'static str)],
) -> Shape {
    let notes: Vec<FieldNote> = fields
        .iter()
        .map(|f| FieldNote {
            optional: optional.contains(&f.key()),
            doc: docs
                .iter()
                .find(|(key, _)| *key == f.key())
                .map_or_else(|| keep(format!("{}.", f.decl().label)), |(_, doc)| doc),
        })
        .collect();
    Shape {
        name,
        doc,
        fields,
        notes: Box::leak(notes.into_boxed_slice()),
    }
}

/// The event type `def` with the struct `name`. The fields with a key in `optional` can be without a value. Every
/// event struct holds the time and the frequency of the event, so a field `time` or `freq_hz` of `def` has no member
/// of its own.
fn event(
    def: &'static EventDef,
    name: &'static str,
    optional: &[&str],
    docs: &[(&str, &'static str)],
) -> EventShape {
    let fields: Vec<&'static dyn Declare> = def
        .fields
        .iter()
        .copied()
        .filter(|f| !["time", "freq_hz"].contains(&f.key()))
        .collect();
    EventShape {
        def,
        fields: shape(
            name,
            keep(format!("{}.", def.label)),
            Box::leak(fields.into_boxed_slice()),
            optional,
            docs,
        ),
    }
}

/// The command `command` with the arguments struct `args` and the reply struct `reply`. The reply fields with a key
/// in `optional` can be without a value.
fn command(
    command: &'static Command,
    args: &'static str,
    reply: &'static str,
    optional: &[&str],
) -> CommandShape {
    let docs: Vec<&'static str> = command
        .args
        .iter()
        .map(|a| {
            let d = a.decl();
            if d.help.is_empty() {
                keep(format!("{}.", d.field.label))
            } else {
                keep(d.help.to_string())
            }
        })
        .collect();
    CommandShape {
        command,
        args: ArgShape {
            name: if command.args.is_empty() { "" } else { args },
            doc: keep(format!("The arguments of `{}`.", command.key)),
            args: command.args,
            docs: Box::leak(docs.into_boxed_slice()),
        },
        reply: shape(
            reply,
            keep(format!("The reply of `{}`.", command.key)),
            command.reply,
            optional,
            &[],
        ),
    }
}

/// The table `def` with the row struct `name`. The columns with a key in `optional` can be without a value.
fn table(def: &'static TableDef, name: &'static str, optional: &[&str]) -> TableShape {
    table_with_docs(def, name, optional, &[])
}

/// As [`table`], with `docs` saying what a column holds, by key. A column without an entry has its label.
fn table_with_docs(
    def: &'static TableDef,
    name: &'static str,
    optional: &[&str],
    docs: &[(&str, &'static str)],
) -> TableShape {
    TableShape {
        def,
        row: shape(
            name,
            keep(format!(
                "A row of the table \"{}\" ({}).",
                def.key, def.label
            )),
            def.columns,
            optional,
            docs,
        ),
    }
}

/// The interface of the kind of object `key`, with the stack `s`.
fn object(key: &'static str, s: StackShape) -> Family {
    Family::object(key, 1, 0).stack(s)
}

/// The interfaces of the session, the radio, the engine, the scan service and the record service, in this order. Each holds what every object of its kind
/// has. A session has the settings, results, events and commands of its stack beside them, and a radio those of its
/// driver.
pub fn object_families() -> &'static [Family] {
    static ALL: OnceLock<Vec<Family>> = OnceLock::new();
    ALL.get_or_init(|| vec![session(), radio(), engine(), scan(), record()])
}

fn session() -> Family {
    // The load tables are new in version 1.1 and the table of connections in 1.2: their paths are those that the shape
    // has with them and not without. The field media_lost is new in 1.3, the fields of the host audio input and the
    // transmit power of a burst in 1.4.
    Family::object("session", 1, 4)
        .stack(session_shape(0))
        .added_in(1, 1, |mut f| {
            f.stacks = vec![session_shape(1)];
            f
        })
        .added(1, 1, &LOAD_FIELDS)
        .added_in(1, 2, |mut f| {
            f.stacks = vec![session_shape(2)];
            f
        })
        .added(1, 3, &["shape.SessionFields.session.media_lost"])
        .added(
            1,
            4,
            &[
                "shape.SessionFields.session.audio_in_lost",
                "shape.SessionFields.session.audio_in_rate_hz",
                "shape.SessionFields.session.audio_in_wanted",
                "shape.TransmitEvent.tx_power_db",
            ],
        )
}

/// The fields of the session that are new in version 1.1.
static LOAD_FIELDS: [&str; 9] = [
    "shape.SessionFields.session.share",
    "shape.SessionFields.session.cores_needed",
    "shape.SessionFields.session.cores_declared",
    "shape.SessionFields.session.cores_free",
    "shape.SessionFields.session.threads",
    "shape.SessionFields.session.stage_behind",
    "shape.SessionFields.session.gaps",
    "shape.SessionFields.session.load_problem",
    "shape.SessionFields.session.core_units",
];

/// The shape of a session: with the tables of components and stages from `tables` 1, and of connections from 2.
fn session_shape(tables: u8) -> StackShape {
    let mut s = StackShape::new("session").settings(SessionSetting::SETTINGS);
    s.fields.push(shape(
        "SessionFields",
        "The state and the counts of a session.",
        &OWN_FIELDS,
        &[
            "session.problem",
            "session.request_problem",
            "session.tx_sent",
            "session.tx_failed",
            "session.tx_last",
            "session.tx_blocked",
            "session.snr_db",
            "session.level_dbfs",
            "session.offset_hz",
            "session.share",
            "session.cores_needed",
            "session.cores_declared",
            "session.cores_free",
            "session.threads",
            "session.stage_behind",
            "session.gaps",
            "session.load_problem",
            "session.core_units",
            "session.audio_in_rate_hz",
        ],
        &[
            ("session.state", "Where the session is."),
            ("session.samples", "The samples that the receiver was fed."),
            ("session.dropped", "The samples that the session lost: those that its queue or a stage could not take, and those missing in gaps that its driver marked. The column `lost` of the table of connections shows the places."),
            ("session.events_unlisted", "The events of a type that the catalog of the engine does not list. They are dropped."),
            ("session.problem", "Why the receiver refused its input, or the last setting that it refused."),
            ("session.request_problem", "Why the engine refused the last request of the receiver."),
            ("session.audio_lost", "The audio samples lost because the host did not take them in time."),
            ("session.transmit_lost", "The transmit samples lost because the host did not take them in time."),
            ("session.retunes", "The changes of frequency that the receiver has taken."),
            ("session.tx_sent", "The bursts that the radio of the session sent."),
            ("session.tx_failed", "The bursts that were refused or did not reach the radio in full."),
            ("session.tx_last", "When the last burst ended."),
            ("session.tx_blocked", "What stops the session from transmitting now."),
            ("session.snr_db", "The signal to noise ratio that the receiver measures."),
            ("session.level_dbfs", "The level that the receiver measures."),
            ("session.offset_hz", "The offset of the signal from the tuned frequency that the receiver measures."),
            ("session.share", "The share of one core that the stack takes now, all its threads together."),
            ("session.cores_needed", "The cores that the stack needs at its rate, measured over the last second."),
            ("session.cores_declared", "The cores of this device that the recorded cost of the stack comes to."),
            ("session.cores_free", "The cores that were free for the session when it started."),
            ("session.threads", "The threads that run the stack."),
            ("session.stage_behind", "The times that a stage found the queue after it full and waited."),
            ("session.gaps", "The places where samples are missing that the stack was told of."),
            ("session.load_problem", "Why the session may lose samples: its stack needs more cores than are free."),
            ("session.core_units", "The calibration units per second that one fast core of this device runs."),
            ("session.media_lost", "The media packets and pictures lost because the host did not take them in time. Decoded video frames are kept for half a second."),
            ("session.audio_in_lost", "The samples of the audio that the host gave to the session and that the graph did not take in time: the oldest go first."),
            ("session.audio_in_rate_hz", "The rate of the audio that the graph of the session takes from the host without conversion, such as a microphone. It has a value while the graph has a host audio input and the session can transmit."),
            ("session.audio_in_wanted", "True while a transmission that sends the host audio runs or push-to-talk is pressed: the host opens its microphone and gives the audio only then."),
        ],
    ));
    if tables >= 1 {
        s.tables
            .push(table(&crate::session::COMPONENTS, "ComponentLoadRow", &[]));
        s.tables.push(table(
            &crate::session::STAGES,
            "StageLoadRow",
            &["declared", "declared_units"],
        ));
    }
    if tables >= 2 {
        s.tables.push(table(
            &crate::session::CONNECTIONS,
            "ConnectionLoadRow",
            &["capacity_bytes", "fill", "wait_mean_us", "wait_worst_us"],
        ));
    }
    s.events.push(event(
        &crate::transmit::TRANSMIT,
        "TransmitEvent",
        &["attenuation", "refused", "tx_power_db"],
        &[
            ("attenuation", "The attenuation of the transmitter during the burst: the negative of `tx_power_db`."),
            ("tx_power_db", "The radio setting `tx_power` during the burst: the output power relative to the full output of the radio."),
        ],
    ));
    s
}

fn radio() -> Family {
    use sdrkit_driver_core as dc;
    let mut s = StackShape::new("radio").settings(RadioSetting::SETTINGS);
    s.fields.push(shape(
        "RadioFields",
        "The link and the counts of a radio.",
        dc::STANDARD_FIELDS,
        &[
            "rate_hz",
            "centre_hz",
            "buffer_pct",
            "radio_lost",
            "link_down",
            "delivered_pct",
            "problem",
        ],
        &[
            ("state", "Where the link to the radio is."),
            ("rate_hz", "The sample rate that the radio provides."),
            ("centre_hz", "The centre frequency of the samples."),
            ("samples", "The samples that the radio delivered."),
            (
                "dropped",
                "The samples dropped because the engine did not keep up.",
            ),
            (
                "buffer_pct",
                "The samples that wait in the buffer, in percent of its size.",
            ),
            (
                "radio_lost",
                "The samples that the radio or its link did not deliver.",
            ),
            ("link_down", "The samples missed while the link was down."),
            (
                "delivered_pct",
                "The delivered rate in percent of the set rate.",
            ),
            ("problem", "The last error of the radio."),
        ],
    ));
    for (def, name) in [
        (&dc::CONNECTED, "ConnectedEvent"),
        (&dc::LOST, "LostEvent"),
        (&dc::OVERFLOW, "OverflowEvent"),
        (&dc::RESTART, "RestartEvent"),
    ] {
        s.events.push(event(def, name, &["source", "text"], &[]));
    }
    for c in <dc::RadioCommand as sdrkit_core::CommandSet>::COMMANDS {
        s.commands.push(CommandShape::from(*c));
    }
    object("radio", s)
}

fn scan() -> Family {
    // The table of hits and the commands start and stop are new in version 1.1. Version 1.2 has the span-block scan
    // and the survey: their settings, status fields and commands, the pass and the text of a hit, the table of
    // candidates, and the plan in the reply of start with its argument `dry`. Version 2.0 replaces the argument `look_s`
    // of survey with the setting look. Version 2.1 has the table of occupied channels.
    Family::object("scan", 2, 1)
        .stack(scan_shape(0))
        // Listed first: the first listing of a path gives its version.
        .added(2, 0, &["scan.setting.look"])
        .added(1, 2, &SCAN_SPAN_PATHS)
        .added_in(1, 1, |mut f| {
            f.stacks = vec![scan_shape(1)];
            f
        })
        .added_in(1, 2, |mut f| {
            f.stacks = vec![scan_shape(2)];
            f
        })
        .added_in(2, 1, |mut f| {
            f.stacks = vec![scan_shape(3)];
            f
        })
}

/// The paths of the scan service that are new in version 1.2 inside items of an earlier version.
static SCAN_SPAN_PATHS: [&str; 17] = [
    "scan.setting.after_find",
    "scan.setting.quiet",
    "scan.setting.reject_hold",
    "shape.ScanFields.time_left_s",
    "shape.ScanFields.pass_s",
    "shape.ScanFields.passes",
    "shape.ScanFields.candidates",
    "shape.ScanFields.watched",
    "shape.HitRow.pass",
    "shape.HitRow.text",
    "shape.StartReply.kind",
    "shape.StartReply.dwell_s",
    "shape.StartReply.pass_s",
    "shape.StartReply.total_s",
    "shape.StartReply.passes",
    "shape.StartReply.visit_s",
    "shape.StartReply.span_pass_s",
];

/// The shape of the scan service at `level`: from 1 with the table of hits and the commands start and stop, from 2
/// with the table of candidates and the commands span, survey and hit.text, from 3 with the table of occupied channels.
fn scan_shape(level: u8) -> StackShape {
    use sdrkit_core::CommandSet;
    use sdrkit_svc_scan as scan;
    let mut s = StackShape::new("scan").settings(scan::ScanSetting::SETTINGS);
    s.fields.push(shape(
        "ScanFields",
        "The state and the progress of the scan service.",
        &scan::STATUS_FIELDS,
        &[
            "index",
            "count",
            "freq_hz",
            "name",
            "stack",
            "at_priority",
            "pass",
            "steps",
            "found",
            "session",
            "total",
            "time_left_s",
            "pass_s",
            "passes",
            "candidates",
            "watched",
        ],
        &[
            ("state", "Where the scan is."),
            ("kind", "The kind of the scan that runs or ran last."),
            ("index", "The place of the channel in the list of the scan."),
            ("count", "The channels of the scan."),
            ("freq_hz", "The frequency of the channel."),
            ("name", "The name of the channel."),
            ("stack", "The stack of the channel."),
            ("at_priority", "The scan listens to the priority channel."),
            ("pass", "The pass over the list."),
            ("steps", "The channels that the scan has visited."),
            ("found", "The channels with a signal so far."),
            ("end", "Why the scan ended."),
            ("session", "The session that the scan tunes."),
            ("total", "The rows of the scan table."),
            (
                "time_left_s",
                "The time until the total time of a span or survey scan is over.",
            ),
            (
                "pass_s",
                "The time that one pass of the scan takes, from its plan.",
            ),
            (
                "passes",
                "The passes that fit into the total time, from the plan of the scan.",
            ),
            (
                "candidates",
                "The candidates of a survey that wait for a visit.",
            ),
            (
                "watched",
                "The value of the watched result of the stack on the block that is held.",
            ),
        ],
    ));
    let find = |key: &str| {
        scan::ScanCommand::COMMANDS
            .iter()
            .copied()
            .find(|c| c.key == key)
            .expect("a command of the scan service")
    };
    if level >= 1 {
        s.tables.push(table(
            &scan::HITS,
            "HitRow",
            &[
                "snr_db",
                "level_dbfs",
                "priority",
                "tone",
                "memory",
                "channel",
                "empty",
            ],
        ));
        for (key, args, reply) in [("start", "StartArgs", "StartReply"), ("stop", "", "")] {
            s.commands.push(command(find(key), args, reply, &[]));
        }
    }
    if level >= 2 {
        s.tables
            .push(table(&scan::CANDIDATES, "CandidateRow", &["skip_until"]));
        for (key, args, reply) in [
            ("span", "SpanArgs", "SpanReply"),
            ("survey", "SurveyArgs", "SurveyReply"),
            ("hit.text", "HitTextArgs", ""),
        ] {
            s.commands.push(command(find(key), args, reply, &[]));
        }
    }
    if level >= 3 {
        s.tables.push(table(&scan::OCCUPIED, "OccupiedRow", &[]));
    }
    s
}

/// The record service: its settings, the tables of running and finished recordings, the commands that start and stop a
/// recording of audio, of IQ samples or of a transport stream, and the command that deletes a recording. The command
/// `delete` is new in version 1.1.
fn record() -> Family {
    Family::object("record", 1, 1)
        .stack(record_shape(false))
        .added_in(1, 1, |mut f| {
            f.stacks = vec![record_shape(true)];
            f
        })
}

/// The shape of the record service, with the command `delete` if `delete`.
fn record_shape(delete: bool) -> StackShape {
    use sdrkit_core::CommandSet;
    use sdrkit_svc_record as rec;
    let mut s = StackShape::new("record").settings(rec::RecordSetting::SETTINGS);
    s.tables.push(table(&rec::RUNNING, "RunningRow", &["file"]));
    s.tables.push(table(
        &rec::RECORDINGS,
        "RecordingRow",
        &["seconds", "rate_hz", "centre_hz", "modified"],
    ));
    for (key, args, reply) in [
        ("audio.start", "AudioStartArgs", "AudioStartReply"),
        ("audio.stop", "AudioStopArgs", ""),
        ("iq.start", "IqStartArgs", "IqStartReply"),
        ("iq.stop", "IqStopArgs", ""),
        ("ts.start", "TsStartArgs", "TsStartReply"),
        ("ts.stop", "TsStopArgs", ""),
        ("delete", "DeleteArgs", ""),
    ] {
        if key == "delete" && !delete {
            continue;
        }
        let c = rec::RecordCommand::COMMANDS
            .iter()
            .copied()
            .find(|c| c.key == key)
            .expect("a command of the record service");
        s.commands.push(command(c, args, reply, &[]));
    }
    s
}

fn engine() -> Family {
    // The fields of the calibration and the load tables of the benchmarks are new in 1.2, and so are three columns of
    // the table of benchmarks. The table of transmit commands and the transmit columns of the tables of stacks, drivers
    // and radios are new in 1.3, the stop command of a transmit command in 1.4, the content `carrier` in 1.5, the host
    // audio channels of a stack in 1.6.
    Family::object("engine", 1, 6)
        .stack(engine_shape(0))
        .added(1, 6, &["shape.StackRow.host_audio_channels"])
        .added(1, 1, &["engine.setting.cores_max"])
        // The table of transmit commands of 1.3 brings this column too: the first listing of a path gives its version.
        .added(1, 4, &["shape.StackTransmitRow.stopped_by"])
        .added_in(1, 2, |mut f| {
            f.stacks = vec![engine_shape(1)];
            f
        })
        .added(
            1,
            2,
            &[
                "shape.BenchmarkRow.threads",
                "shape.BenchmarkRow.thread_cores",
                "shape.BenchmarkRow.status",
            ],
        )
        .added_in(1, 3, |mut f| {
            f.stacks = vec![engine_shape(2)];
            f
        })
        .added(
            1,
            3,
            &[
                "shape.StackRow.transmit",
                "shape.DriverRow.tx_max_rate_hz",
                "shape.RadioRow.transmit",
                "shape.RadioRow.tx_min_hz",
                "shape.RadioRow.tx_max_hz",
                "shape.RadioRow.tx_max_rate_hz",
            ],
        )
}

/// The shape of the engine at `level`: from 1 with the fields of the calibration and the load tables of the
/// benchmarks, from 2 with the table of transmit commands.
fn engine_shape(level: u8) -> StackShape {
    let load = level >= 1;
    let mut s = StackShape::new("engine").settings(EngineSetting::SETTINGS);
    if load {
        s.fields.push(shape(
            "EngineFields",
            "The calibration of the device and the cores of the engine.",
            &eo::ENGINE_FIELDS,
            &[
                "engine.core_units",
                "engine.core_units_slow",
                "engine.cores_fast",
                "engine.cores_slow",
            ],
            &[
                ("engine.core_units", "The calibration units per second that one fast core of this device runs, with the kernels of this build. Absent until the calibration has finished."),
                ("engine.core_units_slow", "The same of one slow core. Equal to the fast core where the platform has one class of cores."),
                ("engine.cores_fast", "The fast cores of this device."),
                ("engine.cores_slow", "The slow cores of this device. 0 where the platform has one class of cores."),
                ("engine.cores_usable", "The cores that the sessions may use: the setting `cores_max`, or the fast cores when it is 0."),
            ],
        ));
    }
    s.events.push(event(
        &crate::request::REQUEST,
        "RequestEvent",
        &["refused", "session"],
        &[],
    ));
    for (c, args, reply, optional) in [
        (&eo::C_SAVE, "", "StateSaveReply", &[][..]),
        (&eo::C_RESTORE, "StateRestoreArgs", "", &[]),
        (&eo::C_BENCH, "BenchmarkRunArgs", "", &[]),
    ] {
        s.commands.push(command(c, args, reply, optional));
    }
    use crate::catalog as c;
    // The tables in the order of the declaration of the engine owner.
    s.tables.push(table_with_docs(
        &c::STACKS,
        "StackRow",
        &[],
        &[(
            c::HOST_AUDIO_CHANNELS.key,
            "The channels of host audio that the graph of the stack takes for a transmission: 0 for none, 1 for mono, 2 for stereo.",
        )],
    ));
    for (def, name, optional) in [
        (&c::SUBMODES, "SubmodeRow", &[][..]),
        (&c::STACK_SIGNALS, "StackSignalRow", &[]),
        (
            &c::DRIVERS,
            "DriverRow",
            &[
                c::MIN_RATE_HZ.key,
                c::MAX_RATE_HZ.key,
                c::TX_MAX_RATE_HZ.key,
            ],
        ),
        (&c::COMPONENT_KINDS, "ComponentKindRow", &[]),
        (&c::SERVICES, "ServiceRow", &[]),
        (
            &c::RADIOS,
            "RadioRow",
            &[c::TX_MIN_HZ.key, c::TX_MAX_HZ.key, c::TX_MAX_RATE_HZ.key],
        ),
        (&c::SESSIONS, "SessionRow", &[c::STACK.key]),
        (
            &crate::request::REQUESTS,
            "RequestRow",
            &["freq_hz", "refused", "session"],
        ),
        (&eo::BENCHMARKS, "BenchmarkRow", &[]),
    ] {
        s.tables.push(table(def, name, optional));
    }
    if load {
        for (def, name, optional) in [
            (&eo::BENCH_COMPONENTS, "BenchmarkComponentRow", &[][..]),
            (&eo::BENCH_STAGES, "BenchmarkStageRow", &["declared_units"]),
            (
                &eo::BENCH_CONNECTIONS,
                "BenchmarkConnectionRow",
                &["capacity_bytes", "wait_mean_us", "wait_worst_us"],
            ),
        ] {
            s.tables.push(table(def, name, optional));
        }
    }
    if level >= 2 {
        s.tables.push(table(
            &c::STACK_TRANSMITS,
            "StackTransmitRow",
            &[c::STOPPED_BY.key],
        ));
    }
    s
}
