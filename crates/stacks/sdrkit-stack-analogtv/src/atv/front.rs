//! The front components of the analogue TV receiver: the centre-spike removal and the mirror. The input guard is the one that
//! `sdrkit_graph::StackGraph` has built in.

use super::{last, Tune};
use sdrkit_core::{Complex32, ConfigError};
use sdrkit_graph::{Component, ComponentInfo, Format, Io, Port};

/// Subtracts a slow complex mean (time constant 0.5 ms): the radio's centre spike does not turn, the carriers do. Bypassed while the
/// vision carrier is at the centre.
#[derive(Default)]
pub struct SpikeRemoval {
    est: Complex32,
    w: f32,
    off: bool,
    /// Removal is never used (NBTV: the carrier is where the spike is).
    pub never: bool,
}

impl SpikeRemoval {
    /// A removal that is always off.
    pub fn bypassed() -> Self {
        SpikeRemoval {
            never: true,
            ..Default::default()
        }
    }
}

impl Component for SpikeRemoval {
    fn values(&self) -> sdrkit_core::Values {
        sdrkit_graph::no_values()
    }
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "atv.spike",
            vec![Port::iq("in"), Port::typed::<Tune>("control")],
            vec![Port::iq("out")],
        )
        .in_place(0, 0)
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.w = (1.0 / (0.0005 * inputs[0].rate.max(1.0))).min(1.0) as f32;
        self.est = Complex32::default();
        self.off = false;
        Ok(vec![inputs[0].clone()])
    }
    fn process(&mut self, io: &mut Io) {
        if let Some(t) = last::<Tune>(io, 1) {
            self.off = t.spike_off;
        }
        let (w, off) = (self.w, self.off || self.never);
        let mut est = self.est;
        for s in io.pass_through::<Complex32>(0, 0) {
            est += (*s - est) * w;
            if !off {
                *s -= est;
            }
        }
        self.est = est;
    }
}

/// Conjugates the input when the carrier was found in the mirrored spectrum (I and Q swapped, or a turned-round system).
#[derive(Default)]
pub struct Mirror {
    on: bool,
}

impl Component for Mirror {
    fn values(&self) -> sdrkit_core::Values {
        sdrkit_graph::no_values()
    }
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "atv.mirror",
            vec![Port::iq("in"), Port::typed::<Tune>("control")],
            vec![Port::iq("out")],
        )
        .in_place(0, 0)
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.on = false;
        Ok(vec![inputs[0].clone()])
    }
    fn process(&mut self, io: &mut Io) {
        if let Some(t) = last::<Tune>(io, 1) {
            self.on = t.mirrored;
        }
        let on = self.on;
        let x = io.pass_through::<Complex32>(0, 0);
        if on {
            for s in x {
                s.im = -s.im;
            }
        }
    }
}
