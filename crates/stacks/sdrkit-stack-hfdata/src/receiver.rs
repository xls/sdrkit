//! What every stack of this crate shares to become a receiver: the section of the test-signal settings, and the receiver of a
//! stack that is one decoder on the 24 kHz channel of the family front end.

use crate::frontend::add_front;
use sdrkit_core::interface::StackShape;
use sdrkit_core::{ConfigError, Receiver, StackConfig};
use sdrkit_graph::{GraphBuilder, StackGraph};

/// The section of the test-signal settings.
pub const GEN_SECTION: &str = "Test signal";

/// The receiver of the stack that `shape` describes: the family front end (the dial at 0 Hz, 24 000 samples/s), then `dec`. The decoder's settings
/// keep their keys, and a `reset` command of the decoder becomes a command of the stack. `name` begins the messages about a
/// sample rate that is too low.
///
/// # Errors
///
/// [`ConfigError`] for a value of `cfg` that a component refuses.
pub fn decoder_receiver<D>(
    shape: &StackShape,
    name: &str,
    dec: D,
    cfg: &StackConfig,
) -> Result<Box<dyn Receiver>, ConfigError>
where
    D: sdrkit_graph::Decoder<Input = sdrkit_core::Complex32>,
{
    let own = dec.settings();
    let reset = dec.commands().iter().any(|c| c.key == "reset");
    let b = add_front(GraphBuilder::new(), "front")
        .catalog(&sdrkit_graph::Catalog::of(cfg)?)
        .add("dec", sdrkit_graph::DecoderComponent::new(dec))
        .input("front")
        .connect("front", "dec.in")
        .output("dec.events")
        .status_from("dec")
        .clock(0.25);
    let mut g = StackGraph::of(shape, b.build()?)
        .min_rate(
            24e3,
            &format!("{name}: the sample rate is too low (24 kHz at least)"),
        )
        .min_rate_live(
            250e3,
            &format!("{name}: the sample rate of a live radio is too low (250 kHz at least)"),
        );
    for s in own.settings() {
        let k = &s.field.key;
        g = g.route(k, &[&format!("dec.{k}")])?;
    }
    g = g.command("clear", sdrkit_graph::CommandTarget::every("clear"));
    if reset {
        g = g.command("reset", sdrkit_graph::CommandTarget::of("dec", "reset"));
    }
    Ok(Box::new(g.with_settings(&cfg.settings).build()?))
}
