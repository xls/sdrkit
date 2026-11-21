//! The impairments of a generator: its clean signal through the `sdrkit-algo-sim` chain, set by the generator settings
//! ([`GenSetting`]). A host wraps a generator with [`impaired`] and adds the generator settings with
//! [`with_generator_settings`].

pub use sdrkit_algo_sim::chain::{is_generator_key, ChainConfig, GenSetting, NoiseKind};
use sdrkit_algo_sim::chain::{Impaired, SignalInfo as SimSignal};
pub use sdrkit_algo_sim::SimError;
use sdrkit_core::settings::SchemaError;
use sdrkit_core::{Generator, Schema, SettingSet, Values};
use std::sync::Arc;

/// The settings of `stack` followed by the generator settings ([`GenSetting`]), as one schema.
///
/// # Errors
///
/// [`SchemaError`] if a key of `stack` is also a generator key.
pub fn with_generator_settings(stack: &Schema) -> Result<Arc<Schema>, SchemaError> {
    let mut all = stack.settings().to_vec();
    all.extend_from_slice(GenSetting::schema().settings());
    Schema::new(all).map(Arc::new)
}

/// What the chain needs to know about a generator's signal: its rate and what it declared (only its rate when it declared
/// nothing, so noise can then be set only over the whole sample rate or as a floor).
pub fn signal_of(gen: &dyn Generator) -> SimSignal {
    let mut s = SimSignal::new(gen.rate_hz());
    if let Some(i) = gen.signal_info() {
        s = s.with_power(i.power).with_bandwidth(i.bandwidth_hz);
        if let Some(r) = i.symbol_rate_hz {
            s = s.with_symbol_rate(r);
        }
        if let Some(r) = i.bit_rate_hz {
            s = s.with_bit_rate(r);
        }
    }
    s
}

/// `gen` through the impairment chain that the generator settings in `settings` describe. Other settings are ignored.
///
/// The schema of `settings` must hold the generator settings ([`with_generator_settings`]).
///
/// # Errors
///
/// [`SimError`] if the chain cannot be made from these values for this signal.
///
/// ```
/// use sdrkit_components::sim::{impaired, with_generator_settings, GenSetting};
/// use sdrkit_core::{Complex32, Generator, Schema, SettingSet, SignalInfo, Value, Values};
///
/// struct Carrier;
/// impl Generator for Carrier {
///     fn rate_hz(&self) -> f64 { 48_000.0 }
///     fn generate(&mut self, out: &mut [Complex32]) -> usize { out.fill(Complex32::new(1.0, 0.0)); out.len() }
///     fn signal_info(&self) -> Option<SignalInfo> { Some(SignalInfo::new(3_000.0)) }
/// }
/// let schema = with_generator_settings(&Schema::of(&[])).unwrap();
/// let mut v = Values::defaults(&schema);
/// v.set(schema.id("gen_offset_hz").unwrap(), &Value::F64(1_000.0)).unwrap();
/// let mut g = impaired(Box::new(Carrier), &v).unwrap();
/// let mut x = vec![Complex32::default(); 480];
/// assert_eq!(g.generate(&mut x), 480);
/// ```
pub fn impaired(
    gen: Box<dyn Generator>,
    settings: &Values,
) -> Result<Box<dyn Generator>, SimError> {
    let sig = signal_of(gen.as_ref());
    let cfg = ChainConfig::from_values(settings, &sig)?;
    Ok(Box::new(Impaired::new(gen, &cfg, sig)?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use sdrkit_core::{Choice, Complex32, SignalInfo, Value};

    struct Tone(f64);
    impl Generator for Tone {
        fn rate_hz(&self) -> f64 {
            self.0
        }
        fn generate(&mut self, out: &mut [Complex32]) -> usize {
            out.fill(Complex32::new(1.0, 0.0));
            out.len()
        }
        fn signal_info(&self) -> Option<SignalInfo> {
            Some(SignalInfo::new(1_000.0).with_symbol_rate(500.0))
        }
    }

    #[test]
    fn the_chain_follows_the_generator_settings_and_refuses_bad_values() {
        let schema = with_generator_settings(&Schema::of(&[])).unwrap();
        let id = |k: &str| schema.id(k).unwrap();
        let mut v = Values::defaults(&schema);
        v.set(id("gen_noise"), &Value::Choice(NoiseKind::Snr.index()))
            .unwrap();
        v.set(id("gen_snr_db"), &Value::F64(0.0)).unwrap();
        v.set(id("gen_seed"), &Value::I64(7)).unwrap();
        let mut g = impaired(Box::new(Tone(10_000.0)), &v).unwrap();
        let mut x = vec![Complex32::default(); 20_000];
        assert_eq!(g.generate(&mut x), x.len());
        // 0 dB SNR in 1 kHz at 10 kHz: noise power 10 over the whole rate, signal 1.
        let p = x.iter().map(|s| s.norm_sqr() as f64).sum::<f64>() / x.len() as f64;
        assert!((p - 11.0).abs() < 0.5, "{p}");
        assert!(v
            .set(id("gen_clock_ppm"), &Value::Text("fast".into()))
            .is_err());
        assert!(is_generator_key("gen_offset_hz") && !is_generator_key("gen_message"));
        assert!(GenSetting::schema().id("gen_snr_db").is_ok());
    }

    #[test]
    fn a_stack_key_that_is_a_generator_key_is_refused() {
        use sdrkit_core::{publish::field, Setting};
        const OWN: Setting<field::F64> = Setting::new("gen_seed", "Seed", "", 0.0);
        assert!(with_generator_settings(&Schema::of(&[&OWN])).is_err());
    }
}
