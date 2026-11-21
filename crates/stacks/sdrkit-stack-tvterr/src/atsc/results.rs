//! What the ATSC 1.0 receiver publishes: the result set of [`crate::typed::AtscFields`], and the tables and events of
//! the transport stream layer.

use sdrkit_core::interface::Fields;
use sdrkit_core::ResultsDecl;

/// What `atsc.report` publishes.
pub fn report_declaration() -> ResultsDecl {
    ResultsDecl::new().with_fields(crate::typed::AtscFields::SHAPE.fields)
}

/// What the ATSC stack publishes.
pub fn declaration() -> ResultsDecl {
    crate::typed::shape(super::ID).results_decl()
}
