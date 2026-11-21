//! `bits.soft`: hard bits to LLRs.

use sdrkit_core::{ConfigError, Values};
use sdrkit_graph::{no_values, Component, ComponentInfo, Format, Io, Kind, Port};
use sdrkit_util_bits::Soft;

/// `bits.soft`: hard bits to LLRs at full confidence (0 is +127, 1 is -127). The output's `llr_scale` is `None`: the values say
/// only the sign.
#[derive(Clone, Copy, Debug, Default)]
pub struct SoftFromHard;

impl Component for SoftFromHard {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "bits.soft",
            vec![Port::hard_bits("in")],
            vec![Port::llr("out").same_format_as(0)],
        )
    }
    fn values(&self) -> Values {
        no_values()
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        Ok(vec![Format {
            llr_scale: None,
            ..inputs[0].with_kind(Kind::Llr)
        }])
    }
    fn process(&mut self, io: &mut Io) {
        let x = io.hard_bits(0);
        io.llrs_out(0).extend(x.iter().map(|&b| i8::from_hard(b)));
    }
}
