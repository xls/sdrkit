//! Types every sdrkit crate shares. See `ARCHITECTURE.md`.
//!
//! A stack crate implements [`Receiver`] (and usually [`Generator`]) and exports a [`StackDescriptor`]. Nothing here depends on
//! threads, radios or the host.
//!
//! - [`stack`]: the stack contract (descriptor, tuning, receiver, generator, output).
//! - [`owner`]: anything a host can configure, read or command ([`Owner`], [`OwnerKey`]).
//! - [`command`]: commands declared once ([`Command`], [`commands!`]), their list ([`Commands`]) and errors ([`CommandError`]).
//! - [`settings`](mod@settings): settings declared once ([`Setting`], [`settings!`]), their [`Schema`] and checked [`Values`].
//! - [`net`]: network connections that the host provides ([`Transport`], [`FakeTransport`](net::FakeTransport)).
//! - [`storage`]: files and folders that the host provides ([`Storage`], [`MemStorage`]).
//! - [`state`]: saved state as an opaque blob ([`StateBlob`]).
//! - [`value`]: the one value type ([`Value`]) and the [`Choice`] trait for Rust enums.
//! - [`record`]: field declarations and records ([`Record`]: one value slot per declared field).
//! - [`results`]: what a receiver publishes ([`Results`]).
//! - [`event`]: discrete results, declared once as an [`EventDef`], with ids from the [`EventCatalog`].
//! - [`media`]: compressed media: codecs and their four-character codes, formats, packets, pictures and video frames.
//! - [`bench`](mod@bench): the benchmark type every crate's `benchmarks()` returns.
//! - [`verify`]: what a stack supplies so a host can verify it (test-signal cases, a comparison of recorded vectors, benchmarks).
//! - [`publish`]: typed result keys ([`Field`]), declared and written from one list ([`Published`]); the standard keys and the
//!   rules for units in keys.
//! - [`station`]: the station context ([`Station`]) every receiver is given.
//! - [`request`]: what a receiver or a service requests from the host ([`HostRequest`]: retune, hop, follow, tune, store,
//!   record).
//! - [`signal`]: the signal types that services tell apart and stacks receive ([`SignalType`]).
//! - [`random`]: the random source that the host provides ([`Random`]).
//! - [`job`]: a long job run on a worker thread, with progress and cancel ([`JobControl`]).
//! - [`codec`]: the audio codec plug-in contract ([`codec::AudioCodec`], [`codec::CodecSet`]).

pub mod backend;
pub mod base64;
pub mod bench;
pub mod codec;
pub mod command;
pub mod event;
pub mod facilities;
pub mod interface;
pub mod job;
pub mod media;
pub mod net;
pub mod operator;
pub mod owner;
pub mod publish;
pub mod random;
pub mod record;
pub mod request;
pub mod results;
pub mod settings;
pub mod signal;
pub mod stack;
pub mod stage;
pub mod state;
pub mod station;
pub mod storage;
pub mod time;
pub mod value;
pub mod verify;

pub use bench::{BenchResult, Benchmark};
pub use command::{Command, CommandDef, CommandError, CommandId, CommandSet, Commands, TxContent};
pub use event::{Event, EventCatalog, EventDecl, EventDef, EventKindId};
pub use facilities::Facilities;
pub use job::{Job, JobControl, JobError};
pub use media::{
    ChannelLayout, Codec, FourCc, MediaFormat, MediaSpec, Packaging, Packet, Picture, PixelFormat,
    PixelLayout, VideoFrame,
};
pub use net::{Conn, Datagram, Listener, NetError, Transport};
pub use operator::{listened_hz, Demand, Operator, Receive, TableOperator, Transmit};
pub use owner::{Owner, OwnerKey, OwnerKeyError};
pub use publish::{CatalogueField, Declare, Field, Published, RasterDef, SeriesDef, TableDef};
pub use random::Random;
pub use record::{ContentType, FieldDecl, FieldId, Record, RecordDecl, RefersTo};
pub use request::{HostRequest, OwnerSetting, RequestError, TuneTarget};
pub use results::{
    Axis, LockState, MapPoint, Raster, RasterDecl, RasterId, Results, ResultsDecl, RowMut, Series,
    SeriesDecl, SeriesId, Status, Table, TableDecl, TableId, TableMut,
};
pub use settings::{
    Argument, CatalogueSetting, Common, CommonSets, DeclareSetting, RateOf, Schema, Setting,
    SettingDecl, SettingError,
    SettingId, SettingSet, Values, ValuesError,
};
pub use signal::SignalType;
pub use stack::{
    AdapterCatalog, AudioPart, AutoGain, ConfigError, Elementary, GenConfig, Generator, InputInfo,
    LockCondition, LockProfile, Lookup, MakeGenerator, MakeReceiver, NameSource, Output, Receiver,
    SignalInfo, StackConfig, StackDescriptor, StackLookup, StageError, StreamKind, SubMode, Tuning,
};
pub use stage::{
    ComponentLoad, ConnectionLoad, CostRow, CostTable, StageDecl, StageLoad, StageRun, StageSetId,
    Staged,
};
pub use state::{StateBlob, StateError};
pub use station::{Station, StationError, StationItem};
pub use storage::{MemStorage, Storage, StorageError, StoreEntry, StorePath, WriteMode};
pub use time::Clock;
pub use value::{Choice, ChoiceDecl, Value, ValueKind};
pub use verify::{CheckLine, VectorRun, VerifyCase, VerifyHooks, VerifyOptions};

/// A complex baseband sample. I and Q are scaled so a full-scale radio provides magnitudes up to about 1.0.
pub type Complex32 = num_complex::Complex<f32>;

/// A frequency or a rate in hertz. `f64` keeps 1 Hz resolution up to the hundreds of gigahertz.
pub type Hz = f64;
