//! Rules on sdrkit events: when an event of the chosen kinds comes and the conditions hold, within the limits, the rule requests
//! its actions. Named areas, the closest approach and schedules produce events that rules handle like any other.

#![warn(missing_docs)]
#![forbid(unsafe_code)]

pub mod automation;
pub mod error;
pub mod events;
pub mod expr;
mod fence;
mod rule;
mod schedule;

pub use automation::{Automation, AutomationCommand, AutomationSetting, Problem, KEY};
pub use error::{AutomationError, TextError};
pub use events::{RecordWhat, AUTOMATION_EVENTS};
pub use expr::{Expr, ExprError, Syntax};
pub use fence::Shape;
pub use rule::ActionKind;
