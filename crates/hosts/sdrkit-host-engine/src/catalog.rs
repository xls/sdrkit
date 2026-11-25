//! The engine as an owner, with the catalogs as its tables, and the owners of the kinds in the catalogs.

use crate::services::ServiceKind;
use crate::session::SessionState;
use crate::EngineError;
use sdrkit_core::interface::{Family, StackShape};
use sdrkit_core::publish::field;
use sdrkit_core::{
    AdapterCatalog, AudioKind, AutoGain, Command, ContentKind, Commands, Facilities, Field, Owner, Published, Receiver,
    ResultsDecl, Schema, SignalType, StackConfig, StackDescriptor, TableDef, TxContent, Values,
};
use sdrkit_driver_core::DriverDescriptor;
use sdrkit_graph::components::HostIn;
use sdrkit_graph::{ComponentDescriptor, Graph, Registry};
use std::sync::{Arc, LazyLock};

/// A name for people.
pub const NAME: Field<field::Text> = Field::new("name", "Name", "");
/// The heading in the stack list.
pub const GROUP: Field<field::Text> = Field::new("group", "Group", "");
/// One sentence about the stack.
pub const BLURB: Field<field::Text> = Field::new("blurb", "About", "");
/// The lowest frequency the stack is tuned to.
pub const MIN_HZ: Field<field::Hz> = Field::new("min_hz", "Lowest frequency", "Hz");
/// The highest frequency the stack is tuned to.
pub const MAX_HZ: Field<field::Hz> = Field::new("max_hz", "Highest frequency", "Hz");
/// The frequency a stack starts on.
pub const DEFAULT_HZ: Field<field::Hz> = Field::new("default_hz", "Frequency", "Hz");
/// The sample rate a stack requests.
pub const RATE_HZ: Field<field::Hz> = Field::new("rate_hz", "Sample rate", "Hz");
/// The lowest sample rate.
pub const MIN_RATE_HZ: Field<field::Hz> = Field::new("min_rate_hz", "Lowest sample rate", "Hz");
/// The highest sample rate.
pub const MAX_RATE_HZ: Field<field::Hz> = Field::new("max_rate_hz", "Highest sample rate", "Hz");
/// The width of the signal.
pub const BANDWIDTH_HZ: Field<field::Hz> = Field::new("bandwidth_hz", "Bandwidth", "Hz");
/// What automatic gain means for the stack.
pub const AUTO_GAIN: Field<field::Choice<AutoGain>> = Field::new("auto_gain", "Automatic gain", "");
/// The stack makes a test signal.
pub const GENERATOR: Field<field::Bool> = Field::new("generator", "Test signal", "");
/// A channel scan waits this long after a retune.
pub const LOCK_SETTLE_S: Field<field::Duration> = Field::new("lock_settle_s", "Settle time", "s");
/// A channel scan leaves a channel with no lock after this time.
pub const LOCK_TIMEOUT_S: Field<field::Duration> =
    Field::new("lock_timeout_s", "Lock timeout", "s");
/// The SNR that a channel scan counts as some signal.
pub const LOCK_SNR_DB: Field<field::F64> = Field::new("lock_snr_db", "Signal SNR", "dB");
/// A channel scan waits this long for a name after the lock.
pub const LOCK_COLLECT_S: Field<field::Duration> = Field::new("lock_collect_s", "Name time", "s");
/// The stack of a row.
pub const STACK: Field<field::Text> = Field::new("stack", "Stack", "").refers_to_table("stacks");
/// A signal type.
pub const SIGNAL: Field<field::Choice<SignalType>> = Field::new("signal", "Signal type", "");
/// It can transmit.
pub const TRANSMIT: Field<field::Bool> = Field::new("transmit", "Transmit", "");
/// The channels of host audio that the graph of a stack takes: 0, 1 or 2.
pub const HOST_AUDIO_CHANNELS: Field<field::I64> =
    Field::new("host_audio_channels", "Host audio channels", "");
/// The kind of content of a stack.
pub const KIND: Field<field::Choice<ContentKind>> = Field::new("kind", "Content kind", "");
/// The kind of sound that a stack gives. No value for a stack without sound.
pub const AUDIO: Field<field::Choice<AudioKind>> = Field::new("audio", "Sound", "");
/// The stack gives still pictures.
pub const PICTURES: Field<field::Bool> = Field::new("pictures", "Pictures", "");
/// The stack gives moving pictures.
pub const VIDEO: Field<field::Bool> = Field::new("video", "Video", "");
/// The stack gives running text for people.
pub const TEXT: Field<field::Bool> = Field::new("text", "Text", "");
/// The key of a command.
pub const COMMAND: Field<field::Text> = Field::new("command", "Command", "");
/// What a command sends on the air.
pub const CONTENT: Field<field::Choice<TxContent>> = Field::new("content", "Content", "");
/// The command that stops the transmission of a command. No value for a transmission that ends by itself.
pub const STOPPED_BY: Field<field::Text> = Field::new("stopped_by", "Stopped by", "");
/// The lowest carrier that the transmitter is specified for.
pub const TX_MIN_HZ: Field<field::Hz> = Field::new("tx_min_hz", "Lowest transmit frequency", "Hz");
/// The highest carrier that the transmitter is specified for.
pub const TX_MAX_HZ: Field<field::Hz> = Field::new("tx_max_hz", "Highest transmit frequency", "Hz");
/// The highest sample rate of the transmitter.
pub const TX_MAX_RATE_HZ: Field<field::Hz> =
    Field::new("tx_max_rate_hz", "Highest transmit rate", "Hz");
/// The number of inputs.
pub const INPUTS: Field<field::I64> = Field::new("inputs", "Inputs", "");
/// The number of outputs.
pub const OUTPUTS: Field<field::I64> = Field::new("outputs", "Outputs", "");
/// The number of settings.
pub const SETTINGS: Field<field::I64> = Field::new("settings", "Settings", "");
/// The driver kind of a radio.
pub const DRIVER: Field<field::Text> =
    Field::new("driver", "Driver", "").refers_to_table("drivers");
/// The address forms of a driver kind, separated by `"; "`.
pub const ADDRESS_FORMS: Field<field::Text> = Field::new("address", "Address", "");
/// The address a radio was opened at.
pub const ADDRESS: Field<field::Text> = Field::new("address", "Address", "");
/// The radio a session reads. Empty for a session that the host feeds.
pub const RADIO: Field<field::Text> = Field::new("radio", "Radio", "").refers_to_table("radios");
/// Where a session is.
pub const STATE: Field<field::Choice<SessionState>> = Field::new("state", "State", "");

const ROWS: usize = 4096;

/// The stacks. The row key is the stack id.
pub static STACKS: TableDef = TableDef {
    key: "stacks",
    label: "Stacks",
    columns: &[
        &NAME,
        &GROUP,
        &BLURB,
        &MIN_HZ,
        &MAX_HZ,
        &DEFAULT_HZ,
        &RATE_HZ,
        &MIN_RATE_HZ,
        &BANDWIDTH_HZ,
        &AUTO_GAIN,
        &GENERATOR,
        &LOCK_SETTLE_S,
        &LOCK_TIMEOUT_S,
        &LOCK_SNR_DB,
        &LOCK_COLLECT_S,
        &TRANSMIT,
        &HOST_AUDIO_CHANNELS,
        &KIND,
        &AUDIO,
        &PICTURES,
        &VIDEO,
        &TEXT,
    ],
    cap: ROWS,
    marks: sdrkit_core::RowMarks {
        label: NAME.key,
        ..sdrkit_core::RowMarks::NONE
    },
};

/// The variants of the stacks. The row key is `<stack>/<variant>`.
pub static SUBMODES: TableDef = TableDef {
    key: "submodes",
    label: "Variants",
    columns: &[&STACK, &NAME, &DEFAULT_HZ],
    cap: ROWS,
    marks: sdrkit_core::RowMarks::NONE,
};

/// The signal types each stack receives. The row key is `<stack>/<signal type>`.
pub static STACK_SIGNALS: TableDef = TableDef {
    key: "stack_signals",
    label: "Signal types",
    columns: &[&STACK, &SIGNAL],
    cap: ROWS,
    marks: sdrkit_core::RowMarks::NONE,
};

/// The driver kinds. The row key is the driver key.
pub static DRIVERS: TableDef = TableDef {
    key: "drivers",
    label: "Drivers",
    columns: &[
        &NAME,
        &ADDRESS_FORMS,
        &MIN_RATE_HZ,
        &MAX_RATE_HZ,
        &TRANSMIT,
        &TX_MAX_RATE_HZ,
        &crate::devices::DriverAvailability::AVAILABLE,
        &crate::devices::DriverAvailability::UNAVAILABLE_WHY,
    ],
    cap: ROWS,
    marks: sdrkit_core::RowMarks::NONE,
};

/// The component kinds. The row key is the kind key.
pub static COMPONENT_KINDS: TableDef = TableDef {
    key: "component_kinds",
    label: "Component kinds",
    columns: &[&INPUTS, &OUTPUTS, &SETTINGS],
    cap: ROWS,
    marks: sdrkit_core::RowMarks::NONE,
};

/// The service kinds. The row key is the service key.
pub static SERVICES: TableDef = TableDef {
    key: "services",
    label: "Services",
    columns: &[&NAME],
    cap: ROWS,
    marks: sdrkit_core::RowMarks::NONE,
};

/// The open radios. The row key is the object key of the radio, `radio.r<n>`.
pub static RADIOS: TableDef = TableDef {
    key: "radios",
    label: "Radios",
    columns: &[
        &DRIVER,
        &ADDRESS,
        &TRANSMIT,
        &TX_MIN_HZ,
        &TX_MAX_HZ,
        &TX_MAX_RATE_HZ,
    ],
    cap: ROWS,
    marks: sdrkit_core::RowMarks::NONE,
};

/// The commands of the stacks that transmit, with what each sends and the command that stops a transmission that lasts.
/// The row key is `<stack>/<command>`. A stack that has a row here can transmit.
pub static STACK_TRANSMITS: TableDef = TableDef {
    key: "stack_transmits",
    label: "Transmit commands",
    columns: &[&STACK, &COMMAND, &CONTENT, &STOPPED_BY],
    cap: ROWS,
    marks: sdrkit_core::RowMarks::NONE,
};

/// The sessions. The row key is the object key of the session, `session.s<n>`.
pub static SESSIONS: TableDef = TableDef {
    key: "sessions",
    label: "Sessions",
    columns: &[&STACK, &RADIO, &STATE],
    cap: ROWS,
    marks: sdrkit_core::RowMarks::NONE,
};

/// `decl` with the catalog tables added.
pub(crate) fn declare(decl: ResultsDecl) -> ResultsDecl {
    [
        &STACKS,
        &SUBMODES,
        &STACK_SIGNALS,
        &DRIVERS,
        &COMPONENT_KINDS,
        &SERVICES,
        &RADIOS,
        &SESSIONS,
    ]
    .into_iter()
    .fold(decl, ResultsDecl::with_table)
}

/// The commands of stack `id` that transmit, from the interface description of its family in `families`. Empty for a
/// stack without a description.
pub(crate) fn transmit_commands(families: &[&'static Family], id: &str) -> Vec<&'static Command> {
    families
        .iter()
        .find_map(|f| f.stack_shape(id))
        .map(|shape| {
            shape
                .commands
                .iter()
                .map(|c| c.command)
                .filter(|c| c.transmits.is_some())
                .collect()
        })
        .unwrap_or_default()
}

/// What the catalogs of the engine list.
#[derive(Clone, Copy)]
pub(crate) struct Sources<'a> {
    /// The stacks.
    pub(crate) stacks: &'a [&'static StackDescriptor],
    /// The host audio channels of each stack of `stacks`, in the same order ([`StackKind::host_audio_channels`]).
    pub(crate) host_audio: &'a [u16],
    /// The interface descriptions of the families of the stacks: what a stack transmits comes from them.
    pub(crate) families: &'a [&'static Family],
    /// The driver kinds.
    pub(crate) drivers: &'a [&'static DriverDescriptor],
    /// The component kinds.
    pub(crate) components: &'a Registry,
    /// The USB bus: a kind of USB radio is as available as the bus.
    pub(crate) usb: &'a dyn sdrkit_driver_usb::UsbBus,
}

/// Writes the rows of the catalogs of `sources` and of the services.
pub(crate) fn fill(p: &mut Published, sources: &Sources<'_>) {
    let Sources {
        stacks,
        host_audio,
        families,
        drivers,
        components,
        usb,
    } = *sources;
    for (i, s) in stacks.iter().enumerate() {
        let t = &s.tuning;
        let l = &s.lock_profile;
        let transmits = transmit_commands(families, s.id);
        for c in &transmits {
            if let Some(mut r) = p.row_mut(&STACK_TRANSMITS, &format!("{}/{}", s.id, c.key)) {
                r.set(&STACK, s.id);
                r.set(&COMMAND, c.key);
                r.opt(&CONTENT, c.transmits);
                r.opt(&STOPPED_BY, c.stopped_by);
            }
        }
        if let Some(mut r) = p.row_mut(&STACKS, s.id) {
            r.set(&NAME, s.name);
            r.set(&GROUP, s.group);
            r.set(&BLURB, s.blurb);
            r.set(&MIN_HZ, t.min_hz);
            r.set(&MAX_HZ, t.max_hz);
            r.set(&DEFAULT_HZ, t.default_hz);
            r.set(&RATE_HZ, t.rate_hz);
            r.set(&MIN_RATE_HZ, t.min_rate_hz);
            r.set(&BANDWIDTH_HZ, t.bandwidth_hz);
            r.set(&AUTO_GAIN, s.auto_gain);
            r.set(&GENERATOR, s.make_generator.is_some());
            r.set(&LOCK_SETTLE_S, l.settle_s);
            r.set(&LOCK_TIMEOUT_S, l.timeout_s);
            r.set(&LOCK_SNR_DB, f64::from(l.signal_snr_db));
            r.set(&LOCK_COLLECT_S, l.collect_s);
            r.set(&TRANSMIT, !transmits.is_empty());
            r.set(
                &HOST_AUDIO_CHANNELS,
                i64::from(host_audio.get(i).copied().unwrap_or(0)),
            );
            // The content kind and the media come from the description of the stack. A stack without a description
            // has no kind and no media.
            let shape = families.iter().find_map(|f| f.stack_shape(s.id));
            let media = shape.map(StackShape::media).unwrap_or_default();
            r.opt(&KIND, shape.and_then(|x| x.content));
            r.opt(&AUDIO, media.audio);
            r.set(&PICTURES, media.pictures);
            r.set(&VIDEO, media.video);
            r.set(&TEXT, media.text);
        }
        for m in s.submodes {
            if let Some(mut r) = p.row_mut(&SUBMODES, &format!("{}/{}", s.id, m.id)) {
                r.set(&STACK, s.id);
                r.set(&NAME, m.name);
                r.set(&DEFAULT_HZ, m.default_hz);
            }
        }
        for g in s.signal_types {
            let key = format!("{}/{}", s.id, sdrkit_core::Choice::key(*g));
            if let Some(mut r) = p.row_mut(&STACK_SIGNALS, &key) {
                r.set(&STACK, s.id);
                r.set(&SIGNAL, *g);
            }
        }
    }
    for d in drivers {
        if let Some(mut r) = p.row_mut(&DRIVERS, d.key) {
            r.set(&NAME, d.name);
            r.set(&ADDRESS_FORMS, &d.addresses.join("; "));
            // A driver whose rates depend on the device behind it declares 0 and `f64::MAX`: the row has no value.
            let rates = d.produces.iter();
            let min = rates.clone().map(|g| g.min_rate_hz).reduce(f64::min);
            let max = rates.map(|g| g.max_rate_hz).reduce(f64::max);
            r.opt(&MIN_RATE_HZ, min.filter(|hz| *hz > 0.0));
            r.opt(&MAX_RATE_HZ, max.filter(|hz| *hz < f64::MAX));
            r.set(&TRANSMIT, !d.accepts.is_empty());
            let tx_max = d.accepts.iter().map(|g| g.max_rate_hz).reduce(f64::max);
            r.opt(
                &TX_MAX_RATE_HZ,
                tx_max.filter(|hz| *hz > 0.0 && *hz < f64::MAX),
            );
            sdrkit_core::interface::Fields::write(&crate::devices::availability(d.key, usb), &mut r);
        }
    }
    for c in components.descriptors() {
        if let Some(mut r) = p.row_mut(&COMPONENT_KINDS, &c.kind) {
            r.set(&INPUTS, c.inputs.len() as i64);
            r.set(&OUTPUTS, c.outputs.len() as i64);
            r.set(&SETTINGS, c.settings.len() as i64);
        }
    }
    for k in ServiceKind::ALL {
        if let Some(mut r) = p.row_mut(&SERVICES, k.key()) {
            r.set(&NAME, k.name());
        }
    }
}

static NO_RESULTS: LazyLock<Arc<ResultsDecl>> = LazyLock::new(Arc::default);
static NO_COMMANDS: LazyLock<Commands> = LazyLock::new(Commands::default);

/// The declarations of a kind, read from one that was made with the defaults.
struct Made {
    results: Arc<ResultsDecl>,
    commands: Commands,
}

/// The most channels of a host audio input (`host.in`) in the graph of `rx`. 0 for a receiver without a graph, or
/// with a graph that has no host audio input.
fn host_audio_channels(rx: &mut dyn Receiver) -> u16 {
    let Some(g) = rx.as_any_mut().and_then(|a| a.downcast_mut::<Graph>()) else {
        return 0;
    };
    g.order()
        .into_iter()
        .filter_map(|id| g.component_as::<HostIn>(id))
        .map(HostIn::channels)
        .max()
        .unwrap_or(0)
}

/// A stack kind as an owner: the schema with its defaults and the declarations of a stack of the kind. It has no values
/// and no results.
pub(crate) struct StackKind {
    desc: &'static StackDescriptor,
    schema: Arc<Schema>,
    made: Option<Made>,
    /// The most host audio channels of the graph of the stack, over its default and its submodes.
    host_audio_channels: u16,
}

impl StackKind {
    pub(crate) fn new(desc: &'static StackDescriptor) -> Self {
        StackKind {
            desc,
            schema: (desc.settings)(),
            made: None,
            host_audio_channels: 0,
        }
    }

    /// Makes a stack of the kind with the defaults once, to read its declarations, and once more for each submode, to
    /// read the host audio channels of its graph. A submode that cannot be made counts 0.
    pub(crate) fn ensure(
        &mut self,
        adapters: &Arc<dyn AdapterCatalog>,
        facilities: &Facilities,
    ) -> Result<(), EngineError> {
        if self.made.is_none() {
            let cfg = StackConfig::new(Values::defaults(&self.schema))
                .with_adapters(Arc::clone(adapters));
            let mut rx = (self.desc.make_receiver)(&cfg)?;
            rx.set_facilities(facilities);
            let submodes = self.desc.submodes.iter().filter_map(|m| {
                let mut rx = (self.desc.make_receiver)(&cfg.clone().with_submode(m.id)).ok()?;
                Some(host_audio_channels(rx.as_mut()))
            });
            self.host_audio_channels = submodes.fold(host_audio_channels(rx.as_mut()), u16::max);
            self.made = Some(Made {
                results: Arc::clone(rx.results_decl()),
                commands: rx.commands().clone(),
            });
        }
        Ok(())
    }

    /// The most host audio channels of the graph of the stack, over its default and its submodes. 0 before
    /// [`StackKind::ensure`] has made the stack.
    pub(crate) fn host_audio_channels(&self) -> u16 {
        self.host_audio_channels
    }
}

impl Owner for StackKind {
    fn schema(&self) -> &Arc<Schema> {
        &self.schema
    }
    fn results_decl(&self) -> &Arc<ResultsDecl> {
        self.made.as_ref().map_or(&NO_RESULTS, |m| &m.results)
    }
    fn commands(&self) -> &Commands {
        self.made.as_ref().map_or(&NO_COMMANDS, |m| &m.commands)
    }
}

/// A component kind as an owner: the schema with its defaults and the declarations of a component of the kind. It has no
/// values and no results.
pub(crate) struct ComponentKind {
    kind: String,
    schema: Arc<Schema>,
    made: Option<Made>,
}

impl ComponentKind {
    pub(crate) fn new(d: &ComponentDescriptor) -> Self {
        ComponentKind {
            kind: d.kind.clone(),
            schema: Arc::clone(&d.settings),
            made: None,
        }
    }

    /// Makes a component of the kind once, to read its declarations.
    pub(crate) fn ensure(&mut self, registry: &Registry) -> Result<(), EngineError> {
        if self.made.is_none() {
            let c = registry.make(&self.kind)?;
            self.made = Some(Made {
                results: c
                    .results()
                    .map_or_else(Arc::default, |r| Arc::clone(r.decl())),
                commands: c.commands().clone(),
            });
        }
        Ok(())
    }
}

impl Owner for ComponentKind {
    fn schema(&self) -> &Arc<Schema> {
        &self.schema
    }
    fn results_decl(&self) -> &Arc<ResultsDecl> {
        self.made.as_ref().map_or(&NO_RESULTS, |m| &m.results)
    }
    fn commands(&self) -> &Commands {
        self.made.as_ref().map_or(&NO_COMMANDS, |m| &m.commands)
    }
}
