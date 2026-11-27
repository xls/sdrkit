//! The automatic modulation of `atv`: with the default settings the stack finds AM broadcast or FM video by itself.

use super::common::*;
use sdrkit_core::owner::Owner;
use sdrkit_core::{Generator, InputInfo, Receiver, SettingSet, StackConfig, Values};
use sdrkit_stack_analogtv::atv::Modulation;
use sdrkit_stack_analogtv::fpv::{FpvGenParams, FpvGenerator};
use sdrkit_stack_analogtv::settings::AtvSetting;
use sdrkit_stack_analogtv::typed::AtvFields;
use sdrkit_stack_analogtv::AtvGenerator;

/// The `atv` receiver of the descriptor with the default settings.
fn default_atv(rate: f64) -> Box<dyn Receiver> {
    let cfg = StackConfig::new(Values::defaults(&AtvSetting::schema()));
    let mut rx = (sdrkit_stack_analogtv::ATV.make_receiver)(&cfg).expect("the receiver builds");
    rx.configure(&InputInfo::new(rate, 2.4e9, 2.4e9).recording())
        .expect("configure");
    rx
}

fn modulation(rx: &dyn Receiver) -> Option<Modulation> {
    Owner::results(rx).fields().at(&AtvFields::MODULATION)
}

fn fm_run(rate: f64, sync_high: bool, imp: Imp, seconds: f64) -> (Run, Option<Modulation>) {
    let mut g = impaired(
        Box::new(FpvGenerator::new(FpvGenParams {
            rate_hz: rate,
            sync_high,
            sound_hz: 0.0,
            ..Default::default()
        })),
        imp,
    );
    let mut rx = default_atv(rate);
    let r = run(rx.as_mut(), rate, seconds, 100_000, |b| {
        g.generate(b);
    });
    (r, modulation(rx.as_ref()))
}

#[test]
fn fm_video_decodes_with_the_default_settings() {
    std::thread::scope(|s| {
        s.spawn(|| {
            let imp = Imp {
                cnr_db: 22.0,
                offset_hz: -2.5e6,
                ..Default::default()
            };
            let (r, m) = fm_run(30e6, false, imp, 3.0);
            assert_eq!(m, Some(Modulation::FmVideo));
            let d = r.last();
            assert_eq!(d.state, 2);
            assert_eq!(d.lines, 625);
            assert!(d.line_count > 0 && d.field_count > 0 && r.pictures > 0);
            assert!(r.first(|d| d.state == 2).unwrap() <= 2.5);
        });
        s.spawn(|| {
            let (r, m) = fm_run(20e6, true, Imp::cnr(30.0), 5.0);
            assert_eq!(m, Some(Modulation::FmVideo));
            let d = r.last();
            assert_eq!(d.state, 2);
            assert_eq!(d.fm_polarity, "sync tip is the highest frequency");
            assert!(r.pictures > 0);
        });
    });
}

#[test]
fn am_and_noise_stay_on_am_with_the_default_settings() {
    std::thread::scope(|s| {
        s.spawn(|| {
            let rate = 16e6;
            let mut g = impaired(
                Box::new(AtvGenerator::new(params(rate))),
                Imp {
                    cnr_db: 30.0,
                    offset_hz: 1.3e6,
                    ..Default::default()
                },
            );
            let mut rx = default_atv(rate);
            let r = run(rx.as_mut(), rate, 3.0, 50_000, |b| {
                g.generate(b);
            });
            assert_eq!(modulation(rx.as_ref()), Some(Modulation::Am));
            assert_eq!(r.last().state, 2);
            assert!(r.pictures > 0);
        });
        s.spawn(|| {
            let rate = 20e6;
            let mut n = noise(0.1, 5);
            let mut rx = default_atv(rate);
            let r = run(rx.as_mut(), rate, 2.0, 100_000, |b| {
                b.fill(Default::default());
                n.process(b);
            });
            assert_eq!(modulation(rx.as_ref()), Some(Modulation::Am));
            assert_ne!(r.last().state, 2);
        });
    });
}

#[test]
fn fm_video_refuses_a_rate_too_low_for_its_picture() {
    let values = values_of(&AtvSetting::schema(), &[("modulation", "fm")]);
    let mut rx = sdrkit_stack_analogtv::atv::graph::receiver(&StackConfig::new(values))
        .expect("the receiver builds");
    assert!(rx
        .configure(&InputInfo::new(4e6, 2.4e9, 2.4e9).recording())
        .is_err());
}
