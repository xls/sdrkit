//! Stacks of private waveforms, each with a receiver, a transmit branch and a test signal: slotted digital voice with a
//! station beacon (`slotted_voice`), a single-carrier QAM link for video, audio and data (`qam_video`), one picture in one
//! burst (`picture_burst`), burst packets with selective repeat (`arq_packet`), one file in LDPC frames with a session
//! protocol (`file_transfer`), and HF weak-signal chat in upper-sideband audio (`hf_chat`). No public document defines these signals; each stack follows a private description and was
//! verified only against its own test vectors.

#![warn(missing_docs)]
// Signal processing walks several arrays by one index.
#![allow(clippy::needless_range_loop, clippy::too_many_arguments)]

pub mod arq_packet;
pub mod family;
pub mod file_transfer;
pub mod hf_chat;
pub mod picture_burst;
pub mod qam_video;
pub mod slotted_voice;
pub mod typed;
pub mod verify;

use sdrkit_core::StackDescriptor;

/// The heading of the stack list.
pub const GROUP: &str = "Custom links";

/// Every stack of this crate.
pub fn descriptors() -> Vec<&'static StackDescriptor> {
    vec![
        &slotted_voice::SLOTTED_VOICE,
        &qam_video::QAM_VIDEO,
        &picture_burst::PICTURE_BURST,
        &arq_packet::ARQ_PACKET,
        &file_transfer::FILE_TRANSFER,
        &hf_chat::HF_CHAT,
    ]
}
