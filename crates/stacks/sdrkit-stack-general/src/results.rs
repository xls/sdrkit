//! The declarations of the results of the general receiver, made from the description in [`crate::typed`].

use crate::typed::{self, AmFields, WHISTLES};
use sdrkit_core::interface::Fields;
use sdrkit_core::ResultsDecl;

/// What the AM processor publishes: its fields and the whistle series.
pub fn am_declaration() -> ResultsDecl {
    ResultsDecl::new()
        .with_fields(AmFields::SHAPE.fields)
        .series(
            WHISTLES.key,
            WHISTLES.label,
            WHISTLES.x_unit,
            WHISTLES.y_unit,
        )
}

/// What the general receiver publishes: the results of its description.
pub fn declaration() -> ResultsDecl {
    typed::shape().results_decl()
}
