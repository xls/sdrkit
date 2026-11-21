//! `dsp.*`: the components over `sdrkit-algo-dsp`: front end, filters, resamplers, analogue demodulators, level, spectrum and
//! the input guard. The types are defined in `sdrkit_graph::components` and re-exported here.

pub use sdrkit_graph::components::{
    AgcComponent, AmDemod, AudioResampler, BiquadComponent, Channel, DcRemoval, Decimate,
    DeemphasisComponent, FirFilter, FmDemod, IqCorrection, NoiseBlanker, NullSink, PowerMeterSink,
    RealToAudio, Resample, Shift, Spectrum, Squelch, SsbDemod,
};
pub use sdrkit_graph::SampleGuard;

use sdrkit_graph::Registry;

/// Adds the `dsp.*` components of `sdrkit-graph` and its standard adapters.
pub fn register(r: &mut Registry) {
    sdrkit_graph::components::register(r);
}

#[cfg(test)]
mod tests {
    use crate::testing::{run_passes, Source};
    use sdrkit_core::Complex32;
    use sdrkit_graph::stream::Data;
    use sdrkit_graph::{Format, GraphBuilder};

    #[test]
    fn the_guard_fixes_bad_samples_in_a_graph() {
        let mut x = vec![Complex32::new(0.5, 0.0); 64];
        x[10] = Complex32::new(f32::NAN, 1.0);
        x[20] = Complex32::new(1e9, 0.0);
        let mut g = GraphBuilder::new()
            .add(
                "src",
                Source::new(Format::iq(1e3, 0.0, 0.0), Data::Iq(x), 16),
            )
            .add("guard", super::SampleGuard::default())
            .connect("src", "guard")
            .build()
            .unwrap();
        let out = run_passes(&mut g, 4, "guard.out");
        let Data::Iq(y) = out else { panic!() };
        assert_eq!(y.len(), 64);
        assert_eq!(y[10], Complex32::new(0.0, 0.0));
        assert!((y[20].norm() - sdrkit_graph::guard::DEFAULT_LIMIT).abs() < 1e-3);
        let r = g.component_results("guard").unwrap();
        assert_eq!(
            r.fields().get(&sdrkit_core::publish::keys::BAD_SAMPLES),
            Some(2)
        );
    }
}
