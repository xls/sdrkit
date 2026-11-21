//! Teletext in the field blanking, the stack descriptors, and cut independence of `mac` and `fpv`.

use super::common::*;
use sdrkit_core::owner::{self, Owner};
use sdrkit_core::{Complex32, GenConfig, InputInfo, Output, Receiver, StackConfig, Values};
use sdrkit_stack_analogtv::systems::SystemId;
use sdrkit_stack_analogtv::AtvGenParams;

#[test]
fn teletext_pages_are_received() {
    std::thread::scope(|s| {
        for (id, cnr) in [(SystemId::G, 0.0), (SystemId::I, 30.0)] {
            s.spawn(move || {
                let p = AtvGenParams {
                    system: id,
                    teletext: true,
                    ..params(10e6)
                };
                let (r, _) = imp_run(
                    p,
                    Imp {
                        cnr_db: cnr,
                        ..Default::default()
                    },
                    1.5,
                    &[("teletext_page", "102")],
                );
                let d = r.last();
                assert_eq!(d.state, 2);
                let t = d.teletext.expect("teletext");
                assert!(t.present);
                assert!(t.packets > 500, "{id:?}: packets {}", t.packets);
                assert_eq!(t.pages, vec!["100", "101", "102"], "{id:?}");
                assert!(t.header.contains("SDRKIT"), "{}", t.header);
                assert_eq!(t.page, "102");
                assert_eq!(
                    t.rows[2].trim(),
                    "ABCDEFGHIJKLMNOPQRSTUVWXYZ",
                    "{id:?}: {:?}",
                    t.rows
                );
                if cnr == 0.0 {
                    assert_eq!(t.errors, 0);
                    assert_eq!(t.parity_errors, 0);
                }
            });
        }
    });
}

#[test]
fn no_teletext_without_it() {
    let (r, _) = gen_run(params(10e6), 1.0, &[]);
    assert!(r.last().teletext.is_none());
}

#[test]
fn the_descriptors_build_their_receivers_and_generators() {
    let ds = sdrkit_stack_analogtv::descriptors();
    let ids: Vec<&str> = ds.iter().map(|d| d.id).collect();
    assert_eq!(ids, vec!["atv", "mac", "fpv"]);
    for d in ds {
        let schema = (d.settings)();
        let values = Values::defaults(&schema);
        assert!(!schema.is_empty());
        let mut rx = (d.make_receiver)(&StackConfig::new(values.clone())).unwrap();
        let rate = d.tuning.rate_hz;
        assert!(d.tuning.min_rate_hz <= rate);
        rx.configure(&InputInfo::new(rate, d.tuning.default_hz, d.tuning.default_hz).recording())
            .unwrap();
        let mut g = (d.make_generator.unwrap())(&GenConfig::new(rate, values.clone()).with_seed(1))
            .unwrap();
        let mut buf = vec![Complex32::default(); 100_000];
        let mut out = Output::default();
        for _ in 0..4 {
            g.generate(&mut buf);
            rx.feed(&buf, &mut out);
        }
        assert!(buf.iter().all(|x| x.re.is_finite() && x.im.is_finite()));
        // Every setting of the stack is accepted.
        for (i, s) in schema.settings().iter().enumerate() {
            owner::apply(rx.as_mut(), sdrkit_core::SettingId(i as u16), &s.default)
                .unwrap_or_else(|e| panic!("{}: {}: {e}", d.id, s.field.key));
        }
        // Too low a rate is refused.
        let mut rx = (d.make_receiver)(&StackConfig::new(values)).unwrap();
        assert!(
            rx.configure(&InputInfo::new(d.tuning.min_rate_hz / 2.0, 1e9, 1e9))
                .is_err(),
            "{}",
            d.id
        );
    }
}

#[test]
fn the_atv_generator_through_its_descriptor_with_settings() {
    let d = &sdrkit_stack_analogtv::ATV;
    let values = values_of(
        &(d.settings)(),
        &[
            ("gen_system", "i"),
            ("gen_second", "nicam_stereo"),
            ("gen_teletext", "true"),
            ("gen_sound", "tone"),
        ],
    );
    let mut g =
        (d.make_generator.unwrap())(&GenConfig::new(10e6, values.clone()).with_seed(3)).unwrap();
    let mut rx = sdrkit_stack_analogtv::atv::graph::receiver(&StackConfig::new(values)).unwrap();
    rx.configure(&InputInfo::new(10e6, 474e6, 474e6).recording())
        .unwrap();
    let r = run(&mut rx, 10e6, 1.25, 65_536, |b| {
        g.generate(b);
    });
    let d = r.last();
    assert_eq!(d.system, "PAL I 625/50");
    assert_eq!(d.sound_system, "NICAM stereo");
    assert!(d.teletext.is_some_and(|t| t.pages.len() == 3));
}

/// The lock state and the decoded field count of a receiver's results.
fn state_of(rx: &dyn Receiver) -> (sdrkit_core::LockState, Option<i64>) {
    let res = Owner::results(rx);
    let f = res.fields();
    let n = f
        .at(&sdrkit_stack_analogtv::typed::FpvFields::FIELDS_OK)
        .or_else(|| f.at(&sdrkit_stack_analogtv::typed::MacFields::FRAMES));
    (res.status().lock_state, n)
}

#[test]
fn mac_and_fpv_do_not_depend_on_the_cuts() {
    for id in ["mac", "fpv"] {
        let d = sdrkit_stack_analogtv::descriptors()
            .into_iter()
            .find(|d| d.id == id)
            .unwrap();
        let mut outs = Vec::new();
        for piece in [4096usize, 100_000] {
            let values = Values::defaults(&(d.settings)());
            let rate = d.tuning.rate_hz;
            let mut g =
                (d.make_generator.unwrap())(&GenConfig::new(rate, values.clone()).with_seed(1))
                    .unwrap();
            let mut rx = (d.make_receiver)(&StackConfig::new(values)).unwrap();
            rx.configure(
                &InputInfo::new(rate, d.tuning.default_hz, d.tuning.default_hz).recording(),
            )
            .unwrap();
            let total = (0.8 * rate) as usize;
            let mut buf = vec![Complex32::default(); piece];
            let mut out = Output::default();
            let mut audio = Vec::new();
            let mut done = 0;
            while done < total {
                let n = piece.min(total - done);
                g.generate(&mut buf[..n]);
                out.clear();
                rx.feed(&buf[..n], &mut out);
                audio.extend_from_slice(out.audio());
                done += n;
            }
            outs.push((state_of(rx.as_ref()), audio));
        }
        let (a, b) = (&outs[0], &outs[1]);
        assert_eq!(a.0, b.0, "{id}");
        assert_eq!(a.0 .0, sdrkit_core::LockState::Locked, "{id}");
        let n = a.1.len().min(b.1.len());
        assert!(n > 10_000, "{id}");
        let diff = a.1[..n]
            .iter()
            .zip(&b.1[..n])
            .map(|(x, y)| (x - y).abs())
            .fold(0.0f32, f32::max);
        assert!(diff < 1e-3, "{id}: audio differs by {diff}");
    }
}
