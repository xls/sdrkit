//! Constructors of the standard components take only values that their schemas accept.

use sdrkit_core::{SettingError, SettingId, Value};
use sdrkit_graph::components::{Channel, Decimate, Shift, Spectrum, SpectrumSize};
use sdrkit_graph::Component;

fn id(c: &dyn Component, key: &str) -> SettingId {
    c.info().settings.id(key).unwrap()
}

#[test]
fn a_value_outside_the_schema_is_refused() {
    let freq = id(&Shift::default(), "freq");
    let err = Shift::new(2e10).err();
    assert!(matches!(err, Some(SettingError::OutOfRange { id, .. }) if id == freq));
    assert!(Decimate::new(0).is_err());
    assert!(
        Channel::new(1.0, 24e3).is_err(),
        "the narrow width is under 1 Hz"
    );
    assert!(Channel::new(10e3, 24e3)
        .unwrap()
        .with_offset(f64::NAN)
        .is_err());
    assert!(Spectrum::new(SpectrumSize::S1024, 0.0).is_err());
}

#[test]
fn a_component_made_with_legal_values_is_described_with_them() {
    let c = Channel::new(12e3, 24e3).unwrap().with_offset(3e3).unwrap();
    let v = c.values();
    assert_eq!(v.value(id(&c, "normal")), Some(&Value::Hz(12e3)));
    assert_eq!(v.value(id(&c, "wide")), Some(&Value::Hz(24e3)));
    assert_eq!(v.value(id(&c, "rate")), Some(&Value::Hz(24e3)));
    assert_eq!(v.value(id(&c, "offset")), Some(&Value::Hz(3e3)));
    let s = Shift::new(-5e3).unwrap();
    assert_eq!(s.values().value(id(&s, "freq")), Some(&Value::Hz(-5e3)));
}

#[test]
fn only_generic_filters_meters_and_sinks_take_several_kinds() {
    let r = sdrkit_graph::Registry::standard();
    let mut several: Vec<&str> = r
        .descriptors()
        .filter(|d| {
            d.inputs.iter().any(|p| {
                matches!(
                    p.format.kind,
                    sdrkit_graph::KindRule::OneOf(_) | sdrkit_graph::KindRule::Any
                )
            })
        })
        .map(|d| d.kind.as_str())
        .collect();
    several.sort_unstable();
    assert_eq!(
        several,
        [
            "dsp.agc",
            "dsp.biquad",
            "dsp.decimator",
            "dsp.deemphasis",
            "dsp.fir",
            "dsp.null_sink",
            "dsp.power_meter",
            "dsp.resampler",
            "dsp.spectrum",
            "dsp.squelch",
            "net.sink",
        ]
    );
}
