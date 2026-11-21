//! Short-range tags and charging: receivers of the load modulation and the reader commands of short-range links. Each stack
//! is a graph of components with typed ports, declares what it publishes, and has a test signal. None transmits.
//!
//! | Id | Stack | Module |
//! |---|---|---|
//! | `rfid_lf` | EM4100 LF tags (100 to 150 kHz) | [`rfid_lf`] |
//! | `rfid_uhf` | EPC Gen2 reader commands (860 to 960 MHz) | [`rfid_uhf`] |
//! | `qi` | Qi wireless charging, power receiver to power transmitter | [`qi`] |

#![warn(missing_docs)]

pub mod bench;
pub mod family;
pub mod qi;
pub mod rfid_lf;
pub mod rfid_uhf;
pub mod typed;

use sdrkit_core::{Results, StackDescriptor, Value};

/// The heading of the stacks in the stack list.
pub const GROUP: &str = "Tags and charging";

/// The stacks of the crate.
pub fn descriptors() -> Vec<&'static StackDescriptor> {
    vec![&rfid_lf::DESCRIPTOR, &rfid_uhf::DESCRIPTOR, &qi::DESCRIPTOR]
}

/// Adds every component of the crate to `r`.
pub fn register(r: &mut sdrkit_graph::Registry) {
    rfid_lf::register(r);
    rfid_uhf::register(r);
    qi::register(r);
}

/// The value of the result field `key` of `r` as a number (a yes is 1); 0 when it has none.
pub fn number(r: &Results, key: &str) -> f64 {
    let Some(v) = r.decl().field_id(key).and_then(|id| r.fields().value(id)) else {
        return 0.0;
    };
    match v {
        Value::I64(x) => *x as f64,
        Value::F64(x) | Value::Hz(x) | Value::Duration(x) | Value::Time(x) => *x,
        Value::Bool(b) => f64::from(u8::from(*b)),
        _ => 0.0,
    }
}
