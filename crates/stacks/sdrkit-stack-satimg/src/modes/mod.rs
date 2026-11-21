//! The stacks, one module each, and the stand-ins used when a receiver or a generator cannot be made.

pub mod apt;
pub mod aws;
pub mod display;
pub mod filecast;
pub mod dsb;
pub mod eodb;
pub mod fy2;
pub mod fy3;
pub mod fy4;
pub mod gac;
pub mod gk2a_cdas;
pub mod glm;
pub mod grb;
pub mod goes_cda;
pub mod goes_mdl;
pub mod gvar;
pub mod hrpt;
pub mod hrpt_meteor;
pub mod meteor;
pub mod msg;
pub mod notes;
pub mod rdas;
pub mod s2;
pub mod sband;
pub mod tables;
pub mod uhrit;
pub mod xrit;
pub mod xrit_gen;

use crate::gen::GenError;
use sdrkit_core::{ConfigError, Receiver, StageError};
use sdrkit_graph::{GraphError, StackGraph};

/// The receiver of a stack graph.
pub fn stack_receiver(
    built: Result<StackGraph, GraphError>,
) -> Result<Box<dyn Receiver>, ConfigError> {
    Ok(Box::new(built?))
}

/// The error of a stack whose test signal refused its options.
pub fn gen_error(e: GenError) -> ConfigError {
    ConfigError::Stage(StageError::new(e))
}
