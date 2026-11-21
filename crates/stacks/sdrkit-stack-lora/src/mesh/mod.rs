//! The mesh receiver and the texting clients: the channel plan, the packet decoder with its
//! tables, the Meshtastic, MeshCore and amateur clients and the transmit scheduler, as plain types the components wrap.

pub mod decode;
pub mod duty;
pub mod ham_client;
pub mod mc_client;
pub mod mt_client;
pub mod plan;
pub mod sched;
pub mod store;
pub mod types;

pub use decode::{McRx, MeshDecoder, MtRx};
pub use plan::{MeshConfig, Plan, PlanEntry};
pub use store::MeshStore;
pub use types::*;

/// A seed from the operating system, for packet ids and back-off times that other stations must not predict. If the system
/// provides none, the seed comes from the clock.
pub fn os_seed() -> u64 {
    let mut b = [0u8; 8];
    if getrandom::getrandom(&mut b).is_err() {
        let t = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos() as u64);
        return t;
    }
    u64::from_le_bytes(b)
}
