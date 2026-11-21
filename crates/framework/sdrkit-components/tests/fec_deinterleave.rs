//! `fec.deinterleave`: the DVB Forney interleaver (EN 300 744 clause 4.3.1, I = 12, M = 17) undone on packets with its
//! end-to-end delay of 2244 bytes, block permutations and delay banks undone on soft bits, frame starts, any cut, and strict
//! settings.

mod fec_support;
use fec_support::*;

use sdrkit_algo_fec::interleave::{dvbs2_bits, DelayLanes, Permutation, ReadOrder};
use sdrkit_components::fec::Deinterleave;
use sdrkit_components::testing::Source;
use sdrkit_core::Value;
use sdrkit_graph::stream::Data;
use sdrkit_graph::{Format, Kind};

#[test]
fn dvb_forney_on_packets_comes_back_after_2244_bytes() {
    let mut rng = Rng(1);
    let data: Vec<Vec<u8>> = (0..40)
        .map(|_| {
            let mut p = rng.bytes(204);
            p[0] = 0x47;
            p
        })
        .collect();
    let mut tx = DelayLanes::forney(12, 17, false, 0u8);
    let sent: Vec<Vec<u8>> = data
        .iter()
        .map(|p| {
            let mut q = p.clone();
            tx.process(&mut q);
            q
        })
        .collect();
    for per_pass in [1usize, 7, 40] {
        let mut g = Tb::new()
            .add(
                "src",
                Source::new(Format::new(Kind::Bytes, 0.0), bytes_data(&sent), per_pass),
            )
            .add("d", Deinterleave::on_bytes())
            .connect("src", "d")
            .build()
            .unwrap();
        let got = run(&mut g, 42, &["d.out"]);
        let out = packets(&got[0].data);
        assert_eq!(out.len(), 40);
        assert!(out.iter().all(|p| p.len() == 204));
        let flat: Vec<u8> = out.concat();
        let want: Vec<u8> = data.concat();
        // 2244 bytes = 11 packets: the sync byte of each packet lands on a sync byte.
        assert_eq!(
            flat[2244..],
            want[..want.len() - 2244],
            "pieces of {per_pass}"
        );
        assert!(out[11..].iter().all(|p| p[0] == 0x47));
    }
}

#[test]
fn block_permutations_on_soft_bits_with_a_frame_start() {
    let mut rng = Rng(2);
    // DVB-S2 8PSK column interleaver (EN 302 307-1 clause 5.3.3) on a 64-bit block in 4 columns, and a 6 x 9 block.
    for (name, extra, p) in [
        (
            "dvbs2",
            vec![("d.size", 64.0), ("d.cols", 4.0)],
            dvbs2_bits(64, 4, false).unwrap(),
        ),
        (
            "rows_cols",
            vec![("d.rows", 6.0), ("d.cols", 9.0)],
            Permutation::row_column(6, 9, ReadOrder::Columns).unwrap(),
        ),
    ] {
        let n = p.len();
        let blocks: Vec<Vec<i8>> = (0..5)
            .map(|_| {
                rng.bits(n)
                    .iter()
                    .map(|&b| if b == 0 { 30 } else { -30 })
                    .collect()
            })
            .collect();
        // Three values of junk first; the frame start then marks the first block.
        let mut stream: Vec<i8> = vec![1, 2, 3];
        for b in &blocks {
            let mut s = vec![0i8; n];
            p.apply(b, &mut s).unwrap();
            stream.extend_from_slice(&s);
        }
        for per_pass in [1usize, 50, 1000] {
            let mut gb = Tb::new()
                .add(
                    "src",
                    TaggedSource::new(
                        Format::llr(100.0, Some(3.0)),
                        Data::Llr(stream.clone()),
                        &[3],
                        per_pass,
                    ),
                )
                .add("d", Deinterleave::default())
                .connect("src", "d")
                .set("d.kind", txt("permutation"))
                .set("d.name", txt(name));
            for (k, v) in &extra {
                gb = gb.set(k, Value::F64(*v));
            }
            let mut g = gb.build().unwrap();
            let got = run(&mut g, stream.len() / per_pass + 2, &["d.out"]);
            let out = llrs(&got[0].data);
            // The partial block before the frame start comes out as erasures; the item count is kept.
            assert_eq!(out.len(), stream.len());
            assert_eq!(out[..3], [0, 0, 0]);
            assert_eq!(
                out[3..],
                blocks.concat()[..],
                "{name}, pieces of {per_pass}"
            );
            assert_eq!(g.format("d.out").unwrap().llr_scale, Some(3.0));
        }
    }
}

#[test]
fn delay_banks_and_custom_forney_on_soft_bits() {
    let mut rng = Rng(3);
    let x: Vec<i8> = rng
        .bits(3000)
        .iter()
        .map(|&b| if b == 0 { 9 } else { -9 })
        .collect();
    // A custom delay bank: the receive side's delays as given.
    let mut bank = DelayLanes::new(&[6, 3, 0], 0i8);
    let mut want = x.clone();
    bank.process(&mut want);
    // DAB time deinterleaving (EN 300 401 clause 12) of 16-bit logical frames.
    let mut dab = DelayLanes::dab_time(16, true, 0i8).unwrap();
    let mut want_dab = x.clone();
    dab.process(&mut want_dab);
    // A Forney deinterleaver of 5 branches, unit 3.
    let mut f = DelayLanes::forney(5, 3, true, 0i8);
    let mut want_f = x.clone();
    f.process(&mut want_f);
    for (settings, want) in [
        (
            vec![
                ("d.kind", "delay"),
                ("d.name", "custom"),
                ("d.delays", "6, 3, 0"),
            ],
            want,
        ),
        (vec![("d.kind", "delay"), ("d.name", "dab_time")], want_dab),
        (vec![("d.kind", "forney"), ("d.name", "custom")], want_f),
    ] {
        for per_pass in [1usize, 77, 3000] {
            let mut gb = Tb::new()
                .add(
                    "src",
                    Source::new(Format::llr(100.0, None), Data::Llr(x.clone()), per_pass),
                )
                .add("d", Deinterleave::default())
                .connect("src", "d")
                .set("d.size", Value::F64(16.0))
                .set("d.branches", Value::F64(5.0))
                .set("d.unit", Value::F64(3.0));
            for (k, v) in &settings {
                gb = gb.set(k, txt(v));
            }
            let mut g = gb.build().unwrap();
            let got = run(&mut g, x.len() / per_pass + 2, &["d.out"]);
            assert_eq!(
                llrs(&got[0].data),
                want,
                "{settings:?}, pieces of {per_pass}"
            );
        }
    }
}

#[test]
fn packets_of_the_wrong_length_for_a_permutation_are_dropped() {
    let mut g = Tb::new()
        .add(
            "src",
            Source::new(
                Format::new(Kind::Bytes, 0.0),
                bytes_data(&[vec![0u8; 704], vec![1u8; 700]]),
                2,
            ),
        )
        .add("d", Deinterleave::on_bytes())
        .connect("src", "d")
        .set("d.kind", txt("permutation"))
        .set("d.name", txt("nicam"))
        .build()
        .unwrap();
    let got = run(&mut g, 2, &["d.out"]);
    assert_eq!(packets(&got[0].data).len(), 1);
    assert_eq!(field(&g, "d", "dropped"), 1.0);
}

#[test]
fn settings_are_strict() {
    let mut d = Deinterleave::default();
    assert!(set(&mut d, "kind", &Value::Text("helical".into())).is_err());
    assert!(set(&mut d, "name", &Value::Text("dvb_t2".into())).is_err());
    assert!(set(&mut d, "branches", &Value::F64(0.0)).is_err());
    assert!(set(&mut d, "delays", &Value::Text("1, two, 3".into())).is_err());
    assert!(set(&mut d, "delays", &Value::Text("1, -2".into())).is_err());
    // A name of another kind, a size that does not fit, a delay bank without delays: refused at configure.
    for settings in [
        vec![
            ("d.kind", Value::Text("forney".into())),
            ("d.name", Value::Text("nicam".into())),
        ],
        vec![
            ("d.kind", Value::Text("permutation".into())),
            ("d.name", Value::Text("bit_reverse".into())),
            ("d.size", Value::F64(12.0)),
        ],
        vec![
            ("d.kind", Value::Text("delay".into())),
            ("d.name", Value::Text("custom".into())),
        ],
        vec![
            ("d.kind", Value::Text("delay".into())),
            ("d.name", Value::Text("dab_time".into())),
            ("d.size", Value::F64(20.0)),
        ],
    ] {
        let mut gb = Tb::new()
            .add(
                "src",
                Source::new(Format::llr(1.0, None), Data::Llr(vec![]), 1),
            )
            .add("d", Deinterleave::default())
            .connect("src", "d");
        for (k, v) in &settings {
            gb = gb.set(k, v.clone());
        }
        let mut g = gb.build().unwrap();
        assert!(
            g.configure_input(Format::iq(1.0, 0.0, 0.0)).is_err(),
            "{settings:?}"
        );
    }
}
