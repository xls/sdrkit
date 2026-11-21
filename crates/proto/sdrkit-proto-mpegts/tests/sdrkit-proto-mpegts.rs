//! The one test program of sdrkit-proto-mpegts. Each module is one area.

#[path = "sdrkit-proto-mpegts/acceptance.rs"]
mod acceptance;
#[path = "sdrkit-proto-mpegts/alloc.rs"]
mod alloc;
#[path = "sdrkit-proto-mpegts/cable.rs"]
mod cable;
#[path = "sdrkit-proto-mpegts/layer.rs"]
mod layer;
#[path = "sdrkit-proto-mpegts/monitor.rs"]
mod monitor;
#[path = "sdrkit-proto-mpegts/mux_file.rs"]
mod mux_file;
#[path = "sdrkit-proto-mpegts/source.rs"]
mod source;
#[path = "sdrkit-proto-mpegts/stream.rs"]
mod stream;
#[path = "sdrkit-proto-mpegts/text.rs"]
mod text;
