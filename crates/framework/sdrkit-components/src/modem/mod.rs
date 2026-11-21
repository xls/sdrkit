//! `modem.*`: the components over `sdrkit-algo-modem`. [`sync`] holds the synchronisation and linear-modulation chain,
//! [`demod`] the demodulators, the sync search and tone detection.

pub mod demod;
pub mod sync;

use sdrkit_graph::Registry;

/// Adds the `modem.*` components.
pub fn register(r: &mut Registry) {
    sync::register(r);
    demod::register(r);
}
