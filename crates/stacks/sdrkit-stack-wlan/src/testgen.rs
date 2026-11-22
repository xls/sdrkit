//! The test-signal settings and errors of the stacks of this crate, and the routing of the other settings.

use sdrkit_core::settings::SettingKind;
use sdrkit_core::{GenConfig, Schema, Setting};
use sdrkit_graph::{GraphError, StackGraphBuilder};
use std::fmt;

/// The section of the test-signal settings.
pub const GEN_SECTION: &str = "Test signal";

/// Why a test signal cannot make a packet or frame.
#[derive(Clone, Debug, PartialEq)]
pub enum GenError {
    /// The physical layer refuses the packet.
    Phy(sdrkit_proto_wlan_phy::Error),
    /// The modulator refuses its parameters.
    Sim(sdrkit_algo_sim::SimError),
    /// The resampler refuses its rates.
    Dsp(sdrkit_algo_dsp::DspError),
}

impl fmt::Display for GenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GenError::Phy(e) => write!(f, "physical layer: {e}"),
            GenError::Sim(e) => write!(f, "modulator: {e}"),
            GenError::Dsp(e) => write!(f, "resampler: {e}"),
        }
    }
}

impl std::error::Error for GenError {}

impl From<sdrkit_proto_wlan_phy::Error> for GenError {
    fn from(e: sdrkit_proto_wlan_phy::Error) -> Self {
        GenError::Phy(e)
    }
}

impl From<sdrkit_algo_sim::SimError> for GenError {
    fn from(e: sdrkit_algo_sim::SimError) -> Self {
        GenError::Sim(e)
    }
}

impl From<sdrkit_algo_dsp::DspError> for GenError {
    fn from(e: sdrkit_algo_dsp::DspError) -> Self {
        GenError::Dsp(e)
    }
}

/// The value of the test-signal setting `s` in `cfg`, `None` if the schema of `cfg` lacks it.
pub fn gen_setting<'a, K: SettingKind>(cfg: &'a GenConfig, s: &Setting<K>) -> Option<K::Out<'a>> {
    let id = cfg.settings.schema().id(s.key()).ok()?;
    s.read(id, cfg.settings.value(id)?).ok()
}

/// Routes every setting of the stack's `schema` that is not a test-signal setting (`gen_*`) or a transmit setting (`tx_*`)
/// to the setting of the same key of `component`.
///
/// # Errors
///
/// [`GraphError::Setting`] if `component` has no setting of that key.
pub fn route_to(
    mut b: StackGraphBuilder,
    component: &str,
    schema: &Schema,
) -> Result<StackGraphBuilder, GraphError> {
    for s in schema.settings() {
        let key = s.field.key.as_ref();
        if !key.starts_with("gen_") && !key.starts_with("tx_") {
            b = b.route(key, &[&format!("{component}.{key}")])?;
        }
    }
    Ok(b)
}
