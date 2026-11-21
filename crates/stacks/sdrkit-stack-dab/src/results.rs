//! The options of the DAB results, and the store of the report component: the result sets and tables of
//! [`crate::typed`] without the playout counts.

use crate::typed;
use sdrkit_core::interface::Fields;
use sdrkit_core::publish::Published;
use sdrkit_core::ResultsDecl;
use sdrkit_proto_bcdata::component::PlayoutFields;
use std::sync::Arc;

sdrkit_core::choice_enum! {
    /// What a service carries.
    pub enum ServiceKind {
        /// DAB+ audio (HE-AAC v2).
        DabPlus = ("dab_plus", "DAB+"),
        /// DAB audio (MPEG Layer II).
        Dab = ("dab", "DAB"),
        /// DMB video.
        Dmb = ("dmb", "DMB"),
        /// Data only.
        Data = ("data", "Data"),
    }
}

sdrkit_core::choice_enum! {
    /// The detection in use.
    pub enum Detection {
        /// With the channel estimate.
        Coherent = ("coherent", "Coherent"),
        /// Differential.
        Differential = ("differential", "Differential"),
    }
}

sdrkit_core::choice_enum! {
    /// The audio codec of the service playing.
    pub enum AudioCodec {
        /// AAC-LC (DAB+ without SBR).
        AacLc = ("aac", "AAC-LC"),
        /// HE-AAC (DAB+ with SBR).
        HeAac = ("he_aac", "HE-AAC"),
        /// HE-AAC v2 (DAB+ with SBR and parametric stereo).
        HeAacV2 = ("he_aac_v2", "HE-AAC v2"),
        /// MPEG-1 or MPEG-2 Audio Layer II.
        Mp2 = ("mp2", "MPEG Layer II"),
    }
}

sdrkit_core::choice_enum! {
    /// What the audio of the service playing does.
    pub enum Playing {
        /// No service is selected.
        Nothing = ("nothing", "No service selected"),
        /// The service is scrambled (conditional access) and cannot play.
        Scrambled = ("scrambled", "Scrambled"),
        /// The sub-channel of the service is not in the ensemble yet.
        Waiting = ("waiting", "Waiting for the sub-channel"),
        /// The sub-channel decodes, but no audio came yet.
        NoAudio = ("no_audio", "No audio yet"),
        /// The audio decodes.
        Decoding = ("decoding", "Decoding"),
    }
}

sdrkit_core::choice_enum! {
    /// The kind of a Journaline object (TS 102 979 clause 5.3).
    pub enum JournalineKind {
        /// A menu.
        Menu = ("menu", "Menu"),
        /// Plain text.
        Text = ("text", "Plain text"),
        /// A title only.
        Title = ("title", "Title only"),
        /// A list.
        List = ("list", "List"),
    }
}

/// A store for what the report component writes: the results of the description without the playout counts, which
/// the playout component writes, and without the events.
pub fn store() -> Published {
    let shape = typed::shape();
    let mut d = ResultsDecl::new();
    for f in shape
        .fields
        .iter()
        .filter(|f| f.name != PlayoutFields::SHAPE.name)
    {
        d = d.with_fields(f.fields);
    }
    for t in &shape.tables {
        d = d.with_table(t.def);
    }
    for s in &shape.series {
        d = d.with_series(s);
    }
    Published::from_decl(Arc::new(d))
}

/// The row key of service `sid`: 4 hexadecimal digits for a programme service, 8 for a data service.
pub fn service_key(sid: u32, out: &mut String) {
    use std::fmt::Write as _;
    out.clear();
    let _ = if sid > 0xFFFF {
        write!(out, "{sid:08X}")
    } else {
        write!(out, "{sid:04X}")
    };
}

/// The id of the services table in the results of the stack, for a command error that names a row of it.
pub fn services_table() -> sdrkit_core::TableId {
    store()
        .results()
        .decl()
        .table_id(typed::SERVICES.key)
        .unwrap_or(sdrkit_core::TableId(0))
}
