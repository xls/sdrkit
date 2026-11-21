//! The tests of the satellite navigation stacks: one module for each area.

mod acceptance;
mod channel;
mod core_fix;
mod cuts;
mod fixes;
mod nav_bits;
mod navic;
mod roundtrip;
mod systems;
mod vector;
mod verify_cases;

use sdrkit_algo_sim::noise::{Awgn, NoiseLevel};
use sdrkit_core::{Complex32, Generator};
use sdrkit_stack_gnss::gen::GnssGenerator;

/// The noise of the standard chain set to the C/N0 of the whole sky (`gen_noise = cn0`), so each satellite of the generator has
/// its own C/N0.
pub struct Sky {
    noise: Awgn,
}

impl Sky {
    /// The noise for `g`.
    pub fn new(g: &GnssGenerator) -> Sky {
        let level = NoiseLevel::Cn0DbHz(g.sky_cn0_dbhz());
        Sky {
            noise: Awgn::complex(level, 1.0, g.opt.rate_hz, 7).unwrap(),
        }
    }

    /// Fills `x` with the next samples of the sky and adds the noise.
    pub fn render(&mut self, g: &mut GnssGenerator, x: &mut [Complex32]) {
        g.generate(x);
        self.noise.process(x);
    }
}
