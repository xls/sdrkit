//! The occupied bandwidth meter: QPSK at two symbol rates, two roll-offs and three offsets; a weak neighbour; a sweep of
//! Es/N0; noise alone. The expected widths come from the raised-cosine spectrum that the generator sends.

use crate::stream::{stream, Streamed};
use sdrkit_algo_sim::chain::{
    NoiseKind, GEN_ESN0_DB, GEN_NOISE, GEN_NOISE_FLOOR_DBFS, GEN_OFFSET_HZ,
};
use sdrkit_core::interface::ReadFields;
use sdrkit_dev_test::Harness;
use sdrkit_stack_meter::obw::gen::{
    rc_share_width, rc_x_db_width, GEN_NEIGHBOUR, GEN_NEIGHBOUR_DB, GEN_ROLLOFF, GEN_SYMBOL_RATE,
};
use sdrkit_stack_meter::obw::DESCRIPTOR;
use sdrkit_stack_meter::typed::{ObwFields, WatchFields};

const CENTRE: f64 = 145e6;

fn psk(rs: f64, a: f64, esn0: f64, offset: f64) -> Harness {
    Harness::new(&DESCRIPTOR)
        .set(&GEN_SYMBOL_RATE, rs)
        .set(&GEN_ROLLOFF, a)
        .set(&GEN_NOISE, NoiseKind::EsN0)
        .set(&GEN_ESN0_DB, esn0)
        .set(&GEN_OFFSET_HZ, offset)
}

fn run(h: Harness, secs: f64, seed: u64) -> Streamed {
    stream(
        &DESCRIPTOR,
        &h.seed(seed),
        1e6,
        CENTRE,
        secs,
        seed,
        |_, _| {},
    )
}

fn fields(r: &Streamed) -> ObwFields {
    ObwFields::read(r.results.fields()).expect("the fields")
}

#[test]
fn qpsk_at_two_rates_two_roll_offs_and_three_offsets() {
    for rs in [25e3, 100e3] {
        for a in [0.35, 0.2] {
            for offset in [0.0, 37.5e3, -81.234e3] {
                let f = fields(&run(psk(rs, a, 40.0, offset), 6.0, 1));
                let at = format!("{rs} Bd, roll-off {a}, {offset} Hz");
                let want99 = rc_share_width(a, 0.99) * rs;
                let want26 = rc_x_db_width(a, 26.0) * rs;
                let (b99, b26, c) = (
                    f.obw_khz.unwrap() * 1e3,
                    f.bw26_khz.unwrap() * 1e3,
                    f.centre_offset_hz.unwrap(),
                );
                println!("{at}: 99 % {b99:.0} ({want99:.0}), 26 dB {b26:.0} ({want26:.0}), centre {c:.1}");
                assert!(
                    (b99 - want99).abs() <= 0.03 * want99 + 300.0,
                    "{at}: 99 % {b99}"
                );
                assert!(
                    (b26 - want26).abs() <= 0.06 * want26 + 500.0,
                    "{at}: 26 dB {b26}"
                );
                assert!((c - offset).abs() <= 0.02 * rs + 100.0, "{at}: centre {c}");
            }
        }
    }
}

#[test]
fn a_neighbour_40_db_weaker_reads_in_the_upper_adjacent_band() {
    let h = psk(25e3, 0.35, 55.0, 0.0)
        .set(&GEN_NEIGHBOUR, true)
        .set(&GEN_NEIGHBOUR_DB, -40.0);
    let f = fields(&run(h, 6.0, 2));
    let (up, low) = (f.adjacent_high_db.unwrap(), f.adjacent_low_db.unwrap());
    println!("adjacent above {up:.2} dB, below {low:.2} dB");
    assert!((up - 40.0).abs() <= 1.5, "{up}");
    assert!(low > 50.0, "{low}");
}

#[test]
fn the_occupied_bandwidth_holds_within_5_percent_from_14_db() {
    let want = rc_share_width(0.35, 0.99) * 25e3;
    for esn0 in [40.0, 30.0, 20.0, 14.0, 10.0, 6.0] {
        let f = fields(&run(psk(25e3, 0.35, esn0, 5e3), 6.0, 3));
        let b = f.obw_khz.map_or(f64::NAN, |b| b * 1e3);
        println!("{esn0} dB: {b:.0} Hz, {want:.0} expected");
        if esn0 >= 14.0 {
            assert!((b - want).abs() <= 0.05 * want, "{esn0} dB: {b}");
        }
    }
}

#[test]
fn noise_alone_is_no_signal() {
    for seed in [4, 5, 6] {
        let h = Harness::new(&DESCRIPTOR)
            .set(&GEN_SYMBOL_RATE, 25e3)
            .set(&GEN_NOISE, NoiseKind::Floor)
            .set(&GEN_NOISE_FLOOR_DBFS, -30.0)
            .set(&sdrkit_algo_sim::chain::GEN_LEVEL_DBFS, -150.0);
        let r = run(h, 8.0, seed);
        let f = fields(&r);
        assert!(!f.signal, "seed {seed}");
        let text = WatchFields::read(r.results.fields())
            .unwrap()
            .text
            .to_string();
        assert!(text.starts_with("no signal"), "seed {seed}: {text}");
    }
}
