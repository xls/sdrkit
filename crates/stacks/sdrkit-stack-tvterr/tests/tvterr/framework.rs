//! Every stack of the crate on the framework: descriptors, declared results, strict settings, declared events, generators that
//! declare their signal. And the shared codes the ISDB-T layers use, with their known answers from ARIB STD-B31.

use super::common;
use sdrkit_algo_fec::conv::{codes, Encoder};
use sdrkit_algo_fec::rs::Rs;
use sdrkit_core::owner::{apply, invoke};
use sdrkit_core::{
    Choice, Complex32, GenConfig, InputInfo, Output, Owner, Receiver, SettingError, Value, Values,
};
use sdrkit_stack_tvterr::isdbt::chain::{BitDelay, LayerRx};
use sdrkit_stack_tvterr::isdbt::fec::{INNER, OUTER};
use sdrkit_stack_tvterr::isdbt::tmcc::LayerParams;

#[test]
fn every_descriptor_is_complete_and_publishes_what_it_declares() {
    let ds = sdrkit_stack_tvterr::descriptors();
    let ids: Vec<&str> = ds.iter().map(|d| d.id).collect();
    assert_eq!(
        ids,
        vec!["isdbt", "isdbtsb", "atsc", "atsc3", "dtmb", "tv-auto"]
    );
    for d in ds {
        assert_eq!(d.id, d.id.to_lowercase());
        assert_eq!(d.group, "TV");
        assert!(d.verify.is_some(), "{} has a verify case", d.id);
        let mut rx = common::receiver(d, &[]);
        rx.configure(&InputInfo::new(
            d.tuning.rate_hz,
            d.tuning.default_hz,
            d.tuning.default_hz,
        ))
        .unwrap_or_else(|e| panic!("{}: the rate it asks for: {e}", d.id));
        let mut out = Output::default();
        rx.feed(&[Complex32::new(0.0, 0.0); 4096], &mut out);
        let decl = sdrkit_stack_tvterr::declaration(d.id).expect("a stack of the crate");
        assert!(!decl.is_empty(), "{} declares its results", d.id);
        // Every field the receiver wrote is declared.
        let r = Owner::results(rx.as_ref());
        for (f, v) in r.fields().iter() {
            if *v != Value::None {
                assert!(
                    decl.field_id(&f.key).is_some(),
                    "{}: {} is not declared",
                    d.id,
                    f.key
                );
            }
        }
        if let Some(g) = d.make_generator {
            let gen = g(&GenConfig::new(d.tuning.rate_hz, Values::defaults(&(d.settings)())).with_seed(1))
            .unwrap();
            assert!(gen.signal_info().is_some(), "{} declares its signal", d.id);
            let x = common::generate(gen, 0.05);
            let p: f64 = x.iter().map(|v| v.norm_sqr() as f64).sum::<f64>() / x.len() as f64;
            assert!((0.7..1.4).contains(&p), "{} generator power {p}", d.id);
        }
    }
}

/// Applies `value` to the stack setting `key` through the checked path.
fn set(rx: &mut dyn Receiver, key: &str, value: &Value) -> Result<(), SettingError> {
    let id = rx.schema().id(key)?;
    apply(rx, id, value)
}

#[test]
fn settings_are_strict() {
    use sdrkit_stack_tvterr::{atsc3, dtmb, isdbt};
    let mut rx = common::receiver(&isdbt::DESCRIPTOR, &[]);
    assert!(
        set(rx.as_mut(), "width", &Value::Choice(9)).is_err(),
        "a width that is not a choice is refused"
    );
    assert!(set(
        rx.as_mut(),
        "width",
        &Value::Choice(isdbt::Width::W8.index())
    )
    .is_ok());
    assert!(set(rx.as_mut(), "nonsense", &Value::F64(1.0)).is_err());
    let mut rx = common::receiver(&isdbt::DESCRIPTOR, &[("width", "8")]);
    assert!(
        rx.configure(&InputInfo::new(7e6, 473e6, 473e6)).is_err(),
        "8 MHz needs 8 Msps"
    );
    rx.configure(&InputInfo::new(10e6, 473e6, 473e6))
        .unwrap_or_else(|e| panic!("{e}"));
    rx.feed(&[Complex32::new(0.0, 0.0); 1000], &mut Output::default());
    assert_eq!(
        Owner::results(rx.as_ref())
            .fields()
            .at(&sdrkit_stack_tvterr::typed::IsdbtFields::WIDTH),
        Some(8e6)
    );
    let mut rx = common::receiver(&dtmb::DESCRIPTOR, &[("width", "6")]);
    assert!(
        rx.configure(&InputInfo::new(6e6, 530e6, 530e6)).is_ok(),
        "6 MHz DTMB works from 6 Msps"
    );
    assert!(set(rx.as_mut(), "width", &Value::Choice(2)).is_err());
    let mut rx = common::receiver(&atsc3::DESCRIPTOR, &[]);
    rx.configure(&InputInfo::new(8e6, 539e6, 539e6))
        .unwrap_or_else(|e| panic!("{e}"));
    let id = rx.commands().id("service.select").expect("the command");
    let mut args = rx.commands().def(id).expect("declared").new_args();
    args.set(&sdrkit_proto_mpegts::publish::SERVICE_ROW.field, "");
    assert!(
        invoke(rx.as_mut(), id, &args).is_ok(),
        "an empty row takes the first service"
    );
    args.set(&sdrkit_proto_mpegts::publish::SERVICE_ROW.field, "1001");
    assert!(
        invoke(rx.as_mut(), id, &args).is_err(),
        "a service that the list does not hold is refused"
    );
    let mut rx = common::receiver(&sdrkit_stack_tvterr::auto::DESCRIPTOR, &[]);
    assert!(set(rx.as_mut(), "satellite", &Value::Text("maybe".into())).is_err());
}

#[test]
fn every_event_is_declared_and_carries_its_frequency() {
    let x = common::generate(
        sdrkit_stack_tvterr::isdbt::gen::IsdbtGenerator::new(Default::default()).unwrap(),
        1.5,
    );
    let mut rx = common::receiver(&sdrkit_stack_tvterr::auto::DESCRIPTOR, &[]);
    let r = common::run(rx.as_mut(), 8e6, &x, 65536, "");
    assert!(!r.events.is_empty());
    let decl = sdrkit_stack_tvterr::declaration("tv-auto").expect("declared");
    for e in &r.events {
        assert!(
            decl.events().iter().any(|d| d.kind == e.kind()),
            "{} is declared",
            e.kind()
        );
        assert!(e.freq_hz > 0.0, "{} is sent with its frequency", e.kind());
    }
}

#[test]
fn the_isdbt_inner_code_is_the_one_of_arib_std_b31() {
    // ARIB STD-B31 clause 3.6.2: G1 = 171 octal (X, sent first) and G2 = 133 octal, read from the newest bit. A single 1
    // through the encoder outputs the generators' taps.
    assert_eq!(INNER, codes::ISDB_T_K7);
    let mut enc = Encoder::new(INNER);
    let mut out = Vec::new();
    enc.encode(&[1, 0, 0, 0, 0, 0, 0], &mut out);
    let x: Vec<u8> = out.iter().step_by(2).copied().collect();
    let y: Vec<u8> = out.iter().skip(1).step_by(2).copied().collect();
    assert_eq!(x, [1, 1, 1, 1, 0, 0, 1]);
    assert_eq!(y, [1, 0, 1, 1, 0, 1, 1]);
}

#[test]
fn a_word_reed_solomon_cannot_correct_is_left_as_it_came() {
    // RS(204,188) corrects 8 bytes. A word with 9 errors must come back unchanged and refused.
    let mut rs = Rs::new(OUTER).unwrap_or_else(|e| panic!("{e}"));
    let mut w = [0u8; 204];
    for (i, b) in w[..188].iter_mut().enumerate() {
        *b = (i * 13 + 5) as u8;
    }
    rs.encode_word(&mut w).unwrap_or_else(|e| panic!("{e}"));
    let mut bad = w;
    for i in [2, 20, 40, 60, 80, 100, 120, 140, 160] {
        bad[i] ^= 0xA5;
    }
    let before = bad;
    assert!(rs.decode(&mut bad, &[]).is_err());
    assert_eq!(bad, before);
    let mut eight = w;
    for i in [2, 20, 40, 60, 80, 100, 120, 140] {
        eight[i] ^= 0x5A;
    }
    assert_eq!(
        rs.decode(&mut eight, &[]).map(|d| d.corrected).ok(),
        Some(8)
    );
    assert_eq!(eight, w);
}

#[test]
fn not_a_number_in_a_layer_does_not_stop_it() {
    // A NaN cell reached the ISDB-T Viterbi decoder, whose path comparison panicked on it.
    let p = LayerParams {
        modulation: 1,
        code_rate: 1,
        interleave: 0,
        segments: 1,
    };
    let mut rx = LayerRx::new(1, p, BitDelay::Frame);
    let l = rx.cells_per_symbol();
    let mut out = Vec::new();
    let bad = vec![(Complex32::new(f32::NAN, f32::INFINITY), 0.1f32); l];
    let nan_var = vec![(Complex32::new(0.5, 0.5), f32::NAN); l];
    for t in 0..2 * 204 {
        rx.push_symbol(if t % 2 == 0 { &bad } else { &nan_var }, t % 204, &mut out);
    }
    assert!(rx.c.viterbi_margin.is_finite());
}
