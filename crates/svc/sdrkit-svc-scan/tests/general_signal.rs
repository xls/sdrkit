//! A survey over the test signal of the general stack, made as the test radio makes it: the generator of the stack
//! through the impairments of its `gen_*` settings, at the rate of the stack, with the station at the tune offset of the
//! stack below the centre of the block.

use sdrkit_algo_dsp::fft::WindowedFft;
use sdrkit_components::sim::with_generator_settings;
use sdrkit_core::{Complex32, GenConfig, SettingSet, Value, Values};
use sdrkit_driver_core::synth::{quantize8, STEP};
use sdrkit_stack_general::{GeneralSetting, DESCRIPTOR};
use sdrkit_svc_scan::*;

const CENTRE_HZ: f64 = 300e6;

/// The settings of the stack and of its test signal at their defaults, overlaid with `set` given as text.
fn values(set: &[(&str, &str)]) -> Values {
    let schema = with_generator_settings(&GeneralSetting::schema()).unwrap();
    let mut v = Values::defaults(&schema);
    for (k, x) in set {
        let id = schema.id(k).unwrap();
        let d = schema.setting(id).unwrap();
        let value = match d.default {
            Value::F64(_) => Value::F64(x.parse().unwrap()),
            Value::Hz(_) => Value::Hz(x.parse().unwrap()),
            Value::Bool(_) => Value::Bool(x.parse().unwrap()),
            Value::Choice(_) => Value::Choice(d.field.choice_index(x).unwrap()),
            _ => Value::Text((*x).into()),
        };
        v.set(id, &value).unwrap();
    }
    v
}

/// The noise of the converter of the test radio: uniform over one step of [`quantize8`].
struct Dither(u64);

impl Dither {
    fn next(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        ((self.0 >> 40) as f32 / (1u64 << 24) as f32 - 0.5) * STEP
    }
}

/// The occupied channels that one look of a survey lists over the test signal of the general stack with `set`.
fn occupied(set: &[(&str, &str)]) -> Vec<OccupiedChannel> {
    let rate = DESCRIPTOR.tuning.rate_hz;
    let v = values(set);
    let make = DESCRIPTOR.make_generator.unwrap();
    let gen = make(&GenConfig::new(rate, v.clone())).unwrap();
    let mut gen = sdrkit_components::sim::impaired(gen, &v).unwrap();
    let half = rate * USED_SHARE / 2.0;
    let mut s = SurveyScanner::new(SurveyScanConfig {
        start_hz: CENTRE_HZ - half,
        stop_hz: CENTRE_HZ + half,
        rate_hz: rate,
        look_s: 2.0,
        reject_hold_s: 0.0,
        find: FindConfig {
            stack: "general".into(),
            watch: "signals".into(),
            above: 0.0,
            settle_s: 0.0,
            dwell_s: 2.0,
            after: AfterFind::Resume,
            quiet_s: 0.0,
            total_s: 0.0,
            clock_offset_s: 0.0,
            hit_cap: 1,
        },
    })
    .unwrap();
    assert_eq!(s.blocks(), [CENTRE_HZ]);
    assert_eq!(s.start(0.0), Step::Tune);
    assert_eq!(s.tick(0.0, &Observation::default(), None), None);
    let need = s.need().unwrap();
    let n = need.bins;
    let frames = (2.0 * rate / n as f64).ceil() as usize;
    let mut fft = WindowedFft::new(n, n, need.window).unwrap();
    let mut x = vec![Complex32::default(); n];
    let mut db = vec![0f32; n];
    let mut dither = Dither(0x5DEE_CE66_D1CE_4E5B);
    for _ in 0..frames {
        assert_eq!(gen.generate(&mut x), n);
        for c in x.iter_mut() {
            *c = Complex32::new(
                quantize8(c.re + dither.next()),
                quantize8(c.im + dither.next()),
            );
        }
        let p = fft.run_power(&x);
        for (k, d) in db.iter_mut().enumerate() {
            *d = 10.0 * p[(k + n / 2) % n].max(1e-30).log10();
        }
        s.frame(&SpectrumFrame {
            time_s: 0.0,
            centre_hz: CENTRE_HZ,
            span_hz: rate,
            bins_db: &db,
        });
    }
    s.tick(1e3, &Observation::default(), None);
    s.occupied().copied().collect()
}

/// The station of the test signal: the tune offset of the stack below the centre of the block.
fn station_hz() -> f64 {
    CENTRE_HZ - DESCRIPTOR.tuning.tune_offset_hz
}

#[test]
fn the_voice_of_the_test_signal_is_an_occupied_channel() {
    let rows = occupied(&[("gen_audio", "voice")]);
    let near: Vec<_> = rows
        .iter()
        .filter(|c| (c.freq_hz - station_hz()).abs() <= 3e3)
        .collect();
    assert_eq!(near.len(), 1, "{rows:?}");
    assert!((3e3..=16e3).contains(&near[0].width_hz), "{rows:?}");
    assert_eq!(rows.len(), 1, "{rows:?}");
}

/// AM voice of the default depth 0.5: a carrier far over its sidebands, listed by the width of the sidebands.
#[test]
fn the_am_voice_of_the_test_signal_is_an_occupied_channel() {
    let rows = occupied(&[("demod", "am"), ("gen_audio", "voice")]);
    assert_eq!(rows.len(), 1, "{rows:?}");
    assert!((rows[0].freq_hz - station_hz()).abs() <= 500.0, "{rows:?}");
    assert!((3e3..=8e3).contains(&rows[0].width_hz), "{rows:?}");
}

/// AM of a steady tone of 1 kHz is a carrier with two lines, which is no channel.
#[test]
fn the_am_tone_of_the_test_signal_is_not_an_occupied_channel() {
    assert_eq!(occupied(&[("demod", "am")]), []);
}

/// FM of a steady tone of 1 kHz is a set of lines 1 kHz apart, which the survey does not take for a channel.
#[test]
fn the_steady_tone_of_the_test_signal_is_not_an_occupied_channel() {
    assert_eq!(occupied(&[]), []);
}
