//! The lines that the verify cases of the stacks check in the results.

use sdrkit_core::verify::CheckLine;
use sdrkit_core::{Results, Value};

/// The number in field `key` of the results, or 0 when it is absent or not a number.
pub fn field(r: &Results, key: &str) -> f64 {
    let Some(v) = r.decl().field_id(key).and_then(|id| r.fields().value(id)) else {
        return 0.0;
    };
    match v {
        Value::I64(x) => *x as f64,
        Value::F64(x) | Value::Hz(x) | Value::Duration(x) | Value::Time(x) => *x,
        _ => 0.0,
    }
}

/// A check line: field `key` is at least `min`. `what` names the count in the line's text.
pub fn at_least(r: &Results, key: &str, min: f64, what: &str) -> CheckLine {
    let v = field(r, key);
    CheckLine::new(key, v >= min, format!("{v} {what} (at least {min})"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use sdrkit_core::interface::{Fields, StackShape};
    use sdrkit_core::publish::Published;
    use std::sync::Arc;

    /// Two fields of the kinds that the checks read.
    #[derive(Fields)]
    struct Probe {
        /// A whole number.
        #[field(label = "A")]
        a: i64,
        /// A number.
        #[field(label = "B")]
        b: f64,
    }

    #[test]
    fn fields_are_read_in_their_typed_form() {
        let decl = StackShape::new("probe").fields::<Probe>().results_decl();
        let mut p = Published::from_decl(Arc::new(decl));
        p.put(&Probe { a: 3, b: 2.5 });
        let r = p.results();
        assert_eq!(super::field(r, "a"), 3.0);
        assert_eq!(super::field(r, "b"), 2.5);
        assert_eq!(super::field(r, "c"), 0.0);
        assert_eq!(at_least(r, "a", 3.0, "x").key, "a");
    }
}
