//! Components, streams, tags and graphs. See ARCHITECTURE.md, "Components and graphs".
//!
//! A [`Component`] does one job through named, typed ports, declares its settings and may publish results. A [`Graph`] checks
//! the connections, negotiates the formats, runs its components on one thread in topological order, and implements
//! [`sdrkit_core::Receiver`].

#![warn(missing_docs)]
#![forbid(unsafe_code)]

pub mod adapt;
mod bench;
pub mod calibrate;
pub mod codec;
pub mod command;
pub mod component;
pub mod components;
pub mod connect;
pub mod context;
pub mod cost;
pub mod decoder;
mod desc;
mod graph;
pub mod guard;
pub mod media;
pub mod meter;
pub mod ordered;
pub mod plan;
pub mod settings;
pub mod stack_graph;
pub mod stream;
mod tap;

pub use adapt::{Adapter, AdapterClass, Catalog, InsertedAdapter, ValidateReport};
pub use bench::benchmarks;
pub use command::{apply_commands, apply_commands_reformat, Command, CommandTarget};
pub use component::{
    checked_values_of, no_values, values_of, Applied, CentreRule, ChannelRule, ChannelsFrom,
    Component, ComponentInfo, FormatConstraint, FrameRule, Io, Kept, KindRule, Latency, OutputRule,
    Port,
    RateFrom, RateRule, Role, SizeRule, SpectrumSpec, StreamSample, TagCarry, TagPolicy,
};
pub use connect::{ComponentHandle, ConnectError, PortRef};
pub use context::{Pool, RunContext, Ticket};
pub use decoder::{
    Decoder, DecoderCommand, DecoderComponent, DecoderFields, DecoderInput, Helper, Outbox,
};
pub use desc::{
    ComponentDesc, ComponentDescriptor, Description, MakeComponent, MediaDecoder, Registry,
};
pub use graph::{
    ComponentOwner, DescPosition, Graph, GraphBuilder, GraphError, OutputProblem,
    PortProblem, SettingTarget, Side, SyntaxError, INPUT,
};
pub use guard::SampleGuard;
pub use media::{
    Codec, Codec2Mode, FourCc, MediaFormat, MediaPacket, MediaSpec, Packaging, Picture, PictureIds,
    PictureState, PixelFormat, PixelLayout, SpeechChannel, SpeechPackaging, SpeechPacket,
    VideoFrame,
};
pub use graph::{Stage, StageProblem, AHEAD_BYTES, MAX_STAGES};
pub use meter::{Meter, SectionTimer};
pub use ordered::InOrder;
pub use plan::GraphPlan;
pub use settings::Every;
pub use stack_graph::{GuardFields, StackGraph, StackGraphBuilder};
pub use stream::walk_tags;
pub use stream::{
    AudioFrames, BitOrder, Buffer, Data, Format, Frames, Item, ItemType, ItemVec, Kind, Packet,
    PacketPool, SampleType, SlicePool, Tag, TagKind, TagSource, TagStep, Timed, TypedTag,
};
pub use tap::TapReader;
