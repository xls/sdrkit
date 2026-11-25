//! The graph components of the stacks, and the control items that a display sends back to the chain.

pub mod fsk;
pub mod gated;
pub mod instruments;
pub mod link;
pub mod pass;
pub mod pm;
pub mod table;
pub mod xrit;

use crate::psk::LinkParams;
use sdrkit_graph::{Io, Item};

/// An instruction from a display to the earlier components of its chain. A display sends it on its `control` output, which a
/// feedback edge joins to the `control` input of each component. A component acts on the variants that concern it and leaves
/// the others.
#[derive(Clone, Debug, PartialEq)]
pub enum Control {
    /// Use this link from now on. The demodulator restarts at the new link.
    Link(LinkParams),
    /// Search the carrier again.
    RestartCarrier,
    /// Search the carrier again and remember the offset as one that never locked.
    RejectCarrier,
    /// Search the code again.
    CodeSearch,
    /// The least input rate of the front end, Hz. Below it the front end measures the level only.
    MinRate(f64),
    /// Measure the symbol rate (on) or stop (off).
    Measure(bool),
}

impl Item for Control {
    const KIND: &'static str = "satimg.control";
}

/// The newest item of type `T` on input `i`.
pub fn last<'a, T: Item>(io: &Io<'a>, i: usize) -> Option<&'a T> {
    io.items::<T>(i).last().map(|t| &t.value)
}

/// The control items waiting on input `i`, oldest first.
pub fn controls<'a>(io: &Io<'a>, i: usize) -> impl Iterator<Item = &'a Control> + 'a {
    io.items::<Control>(i).iter().map(|t| &t.value)
}
