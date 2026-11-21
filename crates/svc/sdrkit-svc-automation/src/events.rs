//! The event kinds of the automation: the actions a rule requests from the engine, the crossings of named areas, the closest
//! approach of a moving object, and the due moments of schedules.

use sdrkit_core::publish::keys;
use sdrkit_core::publish::{field, Declare, Field};
use sdrkit_core::EventDef;

sdrkit_core::choice_enum! {
    /// What a record request records.
    pub enum RecordWhat {
        /// The receiver's audio.
        Audio = ("audio", "Audio"),
        /// The receiver's channel IQ.
        Iq = ("iq", "IQ"),
        /// Both.
        Both = ("both", "Audio and IQ"),
    }
}

/// The rule that made the event (its row key).
pub const RULE: Field<field::Text> = Field::new("rule", "Rule", "");
/// The kind key of the event that started the rule.
pub const TRIGGER: Field<field::Text> = Field::new("trigger", "Trigger", "");
/// The title of a notification.
pub const TITLE: Field<field::Text> = Field::new("title", "Title", "");
/// The stack to tune to. Empty keeps the stack.
pub const STACK: Field<field::Text> = Field::new("stack", "Stack", "");
/// What to record.
pub const WHAT: Field<field::Choice<RecordWhat>> = Field::new("what", "Record", "");
/// A length of time.
pub const SECONDS: Field<field::Duration> = Field::new("seconds", "Length", "s");
/// A size cap, bytes. 0 is the recorder's own cap.
pub const MAX_BYTES: Field<field::I64> = Field::new("max_bytes", "Most bytes", "");
/// The name of a host script.
pub const SCRIPT: Field<field::Text> = Field::new("script", "Script", "");
/// The arguments of a host script, one per line.
pub const ARGS: Field<field::Text> = Field::new("args", "Arguments", "");
/// An output of the outputs service (its row key).
pub const OUTPUT: Field<field::Text> = Field::new("output", "Output", "");
/// A named area (its row key).
pub const FENCE: Field<field::Text> = Field::new("fence", "Area", "");
/// The distance at the closest approach.
pub const CPA_KM: Field<field::F64> = Field::new("cpa_km", "Closest approach", "km");
/// The time to the closest approach.
pub const TCPA: Field<field::Duration> = Field::new("tcpa", "Time to closest approach", "s");
/// A schedule (its row key).
pub const SCHEDULE: Field<field::Text> = Field::new("schedule", "Schedule", "");
/// A name for people.
pub const NAME: Field<field::Text> = Field::new("name", "Name", "");

const GEO_FIELDS: &[&dyn Declare] = &[&FENCE, &keys::SOURCE, &TRIGGER, &keys::POSITION];

/// A rule requests a notification from the host.
pub static NOTIFY: EventDef = EventDef::new(
    "automation.notify",
    "Notification",
    &[
        &RULE,
        &TRIGGER,
        &keys::SOURCE,
        &TITLE,
        &keys::TEXT,
        &keys::URGENCY,
    ],
);
/// A rule requests a tune. The event's frequency is the frequency to tune to.
pub static TUNE: EventDef = EventDef::new(
    "automation.tune",
    "Tune",
    &[&RULE, &TRIGGER, &keys::SOURCE, &STACK],
);
/// A rule requests a recording of the receiver that raised the trigger event.
pub static RECORD: EventDef = EventDef::new(
    "automation.record",
    "Record",
    &[&RULE, &TRIGGER, &keys::SOURCE, &WHAT, &SECONDS, &MAX_BYTES],
);
/// A rule requests a save of the last seconds of the rewind buffer.
pub static REWIND: EventDef = EventDef::new(
    "automation.rewind",
    "Save the rewind",
    &[&RULE, &TRIGGER, &SECONDS],
);
/// A rule requests that the host run a script in its sandbox.
pub static RUN_SCRIPT: EventDef = EventDef::new(
    "automation.script",
    "Run a script",
    &[&RULE, &TRIGGER, &keys::SOURCE, &SCRIPT, &ARGS],
);
/// A rule requests that the engine send the trigger event to one output.
pub static SEND: EventDef = EventDef::new(
    "automation.output",
    "Send to an output",
    &[&RULE, &TRIGGER, &keys::SOURCE, &OUTPUT, &keys::TEXT],
);
/// An object entered a named area.
pub static GEO_ENTER: EventDef = EventDef::new("geo.enter", "Entered an area", GEO_FIELDS);
/// An object left a named area.
pub static GEO_EXIT: EventDef = EventDef::new("geo.exit", "Left an area", GEO_FIELDS);
/// An object comes close to the receiver.
pub static GEO_CPA: EventDef = EventDef::new(
    "geo.cpa",
    "Closest approach",
    &[&keys::SOURCE, &TRIGGER, &keys::POSITION, &CPA_KM, &TCPA],
);
/// A schedule is due. The event's time is the due moment.
pub static SCHEDULE_DUE: EventDef =
    EventDef::new("schedule.due", "Schedule due", &[&SCHEDULE, &NAME]);

/// Every event kind of the automation, for the engine's catalog.
pub static AUTOMATION_EVENTS: [&EventDef; 10] = [
    &NOTIFY,
    &TUNE,
    &RECORD,
    &REWIND,
    &RUN_SCRIPT,
    &SEND,
    &GEO_ENTER,
    &GEO_EXIT,
    &GEO_CPA,
    &SCHEDULE_DUE,
];
