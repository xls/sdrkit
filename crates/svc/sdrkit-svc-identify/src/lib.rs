//! Signal identification: from a capture of complex samples, the measurements of a signal, the signal types that it can be with
//! calibrated probabilities, and a verdict in plain words. A host reads and controls it through [`IdentifyService`] as an owner.

#![warn(missing_docs)]
#![forbid(unsafe_code)]

mod error;
pub mod features;
pub mod identify;
pub mod psd;
pub mod regions;
pub mod scan;
mod service;
pub mod signature;
pub mod synth;
pub mod table;
pub mod tvauto;
pub mod verify;
pub mod voiceauto;

pub use error::IdentifyError;
pub use features::{Channel, Features};
pub use identify::{Calibration, Candidate, Identification, Identifier, Prior, Sureness, Verdict};
pub use regions::Region;
pub use service::{
    IdentifyCommand, IdentifyJob, IdentifyService, IdentifySetting, IdentifyState, OpenLookup,
    OpenWith, Target, CANDIDATES, EVIDENCE, KEY, NOTES, OPEN_SETTINGS, PRIORS, SIGNALS, SYSTEMS,
    VERDICT, WHY,
};
pub use signature::{FrameSync, Keying, ModClass, Signature};
pub use verify::{Check, Detector, Verification};
