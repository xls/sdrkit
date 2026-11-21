//! The C interface of sdrkit for this application: the constants, the structs and the functions of the library
//! crate, under the names of the header `sdrkit.h`.
//!
//! The structs are those that the header is written from, so the application has no copy of a layout. A struct that
//! begins with `size` is made with `Default`, which sets the size. Only `api` uses this module.
#![allow(non_camel_case_types)]

pub use sdrkit::*;

pub type SDRKIT_HANDLE = sdrkit::Handle;
pub type sdrkit_id = sdrkit::Id;
pub type sdrkit_string = sdrkit::Str;
pub type sdrkit_bytes = sdrkit::Bytes;
pub type sdrkit_variant = sdrkit::Variant;
pub type sdrkit_arg = sdrkit::Arg;
pub type sdrkit_command_info = sdrkit::CommandInfo;
pub type sdrkit_decl = sdrkit::Decl;
pub type sdrkit_option = sdrkit::EnumOption;
pub type sdrkit_radio_options = sdrkit::RadioOptions;
pub type sdrkit_stack_options = sdrkit::StackOptions;
pub type sdrkit_event_type_info = sdrkit::EventTypeInfo;
pub type sdrkit_event_info = sdrkit::EventInfo;
pub type sdrkit_results_info = sdrkit::ResultsInfo;
pub type sdrkit_table_info = sdrkit::TableInfo;
pub type sdrkit_snapshot_info = sdrkit::SnapshotInfo;
pub type sdrkit_rows_info = sdrkit::RowsInfo;
pub type sdrkit_audio_info = sdrkit::AudioInfo;
pub type sdrkit_spectrum_frame = sdrkit::SpectrumFrame;
