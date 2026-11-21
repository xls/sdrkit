//! The operator of a session: the one object that decides what the radio of the session must do. A session has exactly one.
//! The host carries out its [`Demand`]; nothing else moves the radio for the session. The graph of the session works on the
//! samples that the radio then delivers and knows no frequency of the radio.

use crate::stack::StackDescriptor;
use crate::Hz;

/// What an operator needs of the receiver of a radio.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Receive {
    /// The frequency that the session listens to. 0 if the operator has none: the radio then keeps its centre, and the
    /// session listens `tune_offset_hz` below it.
    pub tuned_hz: Hz,
    /// The sample rate that the operator would like. 0 leaves the rate to the radio. A radio that does not have the rate
    /// runs the nearest one that it has.
    pub rate_hz: Hz,
    /// The radio is tuned this far above `tuned_hz`, so that the carrier of the radio itself stays off the signal.
    pub tune_offset_hz: Hz,
}

/// The frequency that a session listens to on an input with the centre `centre_hz`: `tuned_hz` if it has one, else
/// `tune_offset_hz` below the centre. This is the one rule for a radio, a recording and a test signal: a session and the
/// centre of its input differ by the tune offset, whether the host gave a frequency or not.
///
/// ```
/// use sdrkit_core::listened_hz;
/// assert_eq!(listened_hz(868.2e6, 868.8e6, 600e3), 868.2e6);
/// assert_eq!(listened_hz(0.0, 868.8e6, 600e3), 868.2e6);
/// ```
pub fn listened_hz(tuned_hz: Hz, centre_hz: Hz, tune_offset_hz: Hz) -> Hz {
    if tuned_hz > 0.0 {
        tuned_hz
    } else {
        centre_hz - tune_offset_hz
    }
}

/// What an operator needs of the transmitter of a radio.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transmit {
    /// The frequency that the transmitter sends on.
    pub centre_hz: Hz,
}

/// What an operator needs of a radio now. `None` leaves that side of the radio as it is.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Demand {
    /// The receiver.
    pub receive: Option<Receive>,
    /// The transmitter. A radio without one ignores it.
    pub transmit: Option<Transmit>,
}

/// The operator of a session.
pub trait Operator: Send + std::fmt::Debug {
    /// What the operator needs of the radio now.
    fn demand(&self) -> Demand;
    /// The host gives the frequency to listen to. 0 follows the centre of the input: the operator then demands no
    /// frequency.
    fn set_frequency(&mut self, hz: Hz);
    /// The frequency that the operator listens to now. 0 if it follows the centre of the input.
    fn frequency(&self) -> Hz;
    /// How far below the centre of its input the operator listens while it follows the input (see [`listened_hz`]).
    fn tune_offset_hz(&self) -> Hz;
    /// The host gives the sample rate that the stack would like with its present settings
    /// ([`StackDescriptor::rate_hz`]). The operator demands it from then on. The default keeps the rate of the
    /// operator.
    fn set_rate(&mut self, hz: Hz) {
        let _ = hz;
    }
}

/// The operator of a stack that a table describes: it listens to one frequency, the default of the table until the host
/// gives another, at the rate and the offset of the table.
#[derive(Clone, Debug, PartialEq)]
pub struct TableOperator {
    rate_hz: Hz,
    tune_offset_hz: Hz,
    tuned_hz: Hz,
}

impl TableOperator {
    /// The operator of `stack` in `submode` (`None` for its default). It starts on the default frequency of the submode,
    /// else of the stack.
    pub fn of_stack(stack: &StackDescriptor, submode: Option<&str>) -> TableOperator {
        let default_hz = submode
            .and_then(|m| stack.submodes.iter().find(|s| s.id == m))
            .map_or(stack.tuning.default_hz, |s| s.default_hz);
        TableOperator {
            rate_hz: stack.tuning.rate_hz,
            tune_offset_hz: stack.tuning.tune_offset_hz,
            tuned_hz: default_hz,
        }
    }

    /// An operator without a table, for a session whose host decides everything: it follows the centre of the input until
    /// the host gives a frequency, and it asks for no rate and no offset.
    pub fn manual() -> TableOperator {
        TableOperator {
            rate_hz: 0.0,
            tune_offset_hz: 0.0,
            tuned_hz: 0.0,
        }
    }
}

impl Operator for TableOperator {
    /// Without a frequency the operator still demands its rate, and no transmit frequency.
    fn demand(&self) -> Demand {
        let tuned = self.tuned_hz > 0.0;
        Demand {
            receive: (tuned || self.rate_hz > 0.0).then_some(Receive {
                tuned_hz: self.tuned_hz,
                rate_hz: self.rate_hz,
                tune_offset_hz: self.tune_offset_hz,
            }),
            transmit: tuned.then_some(Transmit {
                centre_hz: self.tuned_hz,
            }),
        }
    }

    fn set_frequency(&mut self, hz: Hz) {
        self.tuned_hz = hz.max(0.0);
    }

    fn frequency(&self) -> Hz {
        self.tuned_hz
    }

    fn set_rate(&mut self, hz: Hz) {
        self.rate_hz = hz.max(0.0);
    }

    fn tune_offset_hz(&self) -> Hz {
        self.tune_offset_hz
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_manual_operator_demands_nothing_until_the_host_gives_a_frequency() {
        let mut op = TableOperator::manual();
        assert_eq!(op.demand(), Demand::default());
        op.set_frequency(145.5e6);
        let d = op.demand();
        assert_eq!(
            d.receive,
            Some(Receive {
                tuned_hz: 145.5e6,
                rate_hz: 0.0,
                tune_offset_hz: 0.0
            })
        );
        assert_eq!(d.transmit, Some(Transmit { centre_hz: 145.5e6 }));
        // Back to the centre of the input.
        op.set_frequency(0.0);
        assert_eq!((op.frequency(), op.demand()), (0.0, Demand::default()));
    }

    #[test]
    fn an_operator_with_no_frequency_demands_its_rate_and_no_frequency() {
        let mut op = TableOperator {
            rate_hz: 2e6,
            tune_offset_hz: 600e3,
            tuned_hz: 0.0,
        };
        let rx = Receive {
            tuned_hz: 0.0,
            rate_hz: 2e6,
            tune_offset_hz: 600e3,
        };
        assert_eq!(
            op.demand(),
            Demand {
                receive: Some(rx),
                transmit: None
            }
        );
        assert_eq!(
            listened_hz(op.frequency(), 868.8e6, op.tune_offset_hz()),
            868.2e6
        );
        op.set_frequency(868.2e6);
        assert_eq!(op.demand().transmit, Some(Transmit { centre_hz: 868.2e6 }));
        assert_eq!(
            listened_hz(op.frequency(), 900e6, op.tune_offset_hz()),
            868.2e6
        );
    }

    #[test]
    fn an_operator_demands_the_rate_that_the_host_gives() {
        let mut op = TableOperator {
            rate_hz: 64e6 / 7.0,
            tune_offset_hz: 0.0,
            tuned_hz: 0.0,
        };
        op.set_rate(8e6);
        assert_eq!(op.demand().receive.map(|r| r.rate_hz), Some(8e6));
    }
}
