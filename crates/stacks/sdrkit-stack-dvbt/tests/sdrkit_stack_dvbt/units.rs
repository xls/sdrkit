//! The error-correction units: the whole coding chain without OFDM, and the puncturing phase search.

use crate::common::check_test_packet;
use sdrkit_core::Complex32;
use sdrkit_dev_test::Rng;
use sdrkit_proto_mpegts::source::NumberedSource;
use sdrkit_stack_dvbt::fecstage::FecChain;
use sdrkit_stack_dvbt::generator::PATTERN_HP;
use sdrkit_stack_dvbt::generator::{DvbtGenerator, GenParams};
use sdrkit_stack_dvbt::items::DataCells;
use sdrkit_stack_dvbt::params::*;
use std::sync::Arc;

/// Cells of `symbols` symbols with AWGN at Es/N0 `esn0_db`, through the inner and outer decoders: the delivered packet counters.
fn chain(
    tps: TpsParams,
    symbols: usize,
    esn0_db: f64,
    phase: usize,
) -> (Vec<Option<u32>>, Option<usize>) {
    let mut p = GenParams::new(Bandwidth::B8, tps);
    p.hp = Box::new(NumberedSource::new(PATTERN_HP));
    p.puncture_phase = phase;
    let mut g = DvbtGenerator::new(p);
    let var = 10f64.powf(-esn0_db / 10.0);
    let sigma = (var / 2.0).sqrt();
    let mut rng = Rng::new(99);
    let mut fec = FecChain::new();
    let mut pk = Vec::new();
    for _ in 0..symbols {
        let (mut cells, l) = g.next_cells();
        for c in cells.iter_mut() {
            *c += Complex32::new((rng.gauss() * sigma) as f32, (rng.gauss() * sigma) as f32);
        }
        let nd = cells.len();
        let d = DataCells {
            cells: Arc::from(cells),
            noise: Arc::from(vec![(var / 2.0) as f32; nd]),
            l,
            frame: 0,
            tps_word: None,
            params: tps,
            erased: false,
            fresh: false,
            time: 0.0,
            rate: Bandwidth::B8.natural_rate(),
        };
        fec.push(&d, &mut pk);
    }
    (
        pk.iter().map(|p| check_test_packet(&p.0, 0x0100)).collect(),
        fec.inner.stats.phase,
    )
}

#[test]
fn f2_the_coding_chain_at_its_unit_thresholds() {
    let cases = [
        (Mode::K2, Constellation::Qpsk, CodeRate::R12, 6.0),
        (Mode::K2, Constellation::Qpsk, CodeRate::R56, 12.0),
        (Mode::K2, Constellation::Qam16, CodeRate::R23, 16.0),
        (Mode::K2, Constellation::Qam64, CodeRate::R34, 23.0),
        (Mode::K2, Constellation::Qam64, CodeRate::R78, 28.0),
        (Mode::K8, Constellation::Qam64, CodeRate::R23, 20.0),
        (Mode::K8, Constellation::Qam16, CodeRate::R12, 11.0),
    ];
    for (mode, c, r, es) in cases {
        let tps = TpsParams {
            mode,
            guard: Guard::G8,
            constellation: c,
            rate_hp: r,
            rate_lp: r,
            ..Default::default()
        };
        let symbols = if mode == Mode::K2 { 272 } else { 136 };
        let (pk, _) = chain(tps, symbols, es, 0);
        let sent = (symbols * mode.data_carriers() * c.bits()) as f64 * r.value() / 8.0 / 204.0;
        let all_good = pk.iter().all(|p| p.is_some());
        let counts: Vec<u32> = pk.iter().flatten().copied().collect();
        let contiguous = counts.windows(2).all(|w| w[1] == w[0] + 1);
        eprintln!("F2 {} {} {} at {es} dB: {} packets of {:.0} sent, all good {all_good}, contiguous {contiguous}", mode.text(), c.text(), r.text(), pk.len(), sent);
        assert!(
            all_good && contiguous && pk.len() as f64 > sent / 2.0,
            "{mode:?} {c:?} {r:?}"
        );
    }
}

#[test]
fn f3_the_puncturing_phase_is_found() {
    for r in [CodeRate::R23, CodeRate::R34, CodeRate::R56, CodeRate::R78] {
        for phase in 0..r.ratio().1 {
            let tps = TpsParams {
                mode: Mode::K2,
                guard: Guard::G8,
                constellation: Constellation::Qam16,
                rate_hp: r,
                rate_lp: r,
                ..Default::default()
            };
            let (pk, found) = chain(tps, 80, 22.0, phase);
            let good = pk.iter().filter(|p| p.is_some()).count();
            assert_eq!(found, Some(phase), "{r:?} phase {phase}");
            assert!(
                good > 100 && good == pk.len(),
                "{r:?} phase {phase}: {good} of {}",
                pk.len()
            );
        }
    }
}

/// After a fresh start in the middle of the stream (a resynchronisation), with erased symbols first, the chain finds the phase and the
/// packets again.
#[test]
fn the_chain_recovers_after_a_fresh_start_with_erased_symbols() {
    let tps = TpsParams {
        mode: Mode::K8,
        guard: Guard::G16,
        constellation: Constellation::Qam64,
        rate_hp: CodeRate::R34,
        rate_lp: CodeRate::R34,
        ..Default::default()
    };
    let mut p = GenParams::new(Bandwidth::B8, tps);
    p.hp = Box::new(NumberedSource::new(PATTERN_HP));
    let mut g = DvbtGenerator::new(p);
    let mut fec = FecChain::new();
    let mut pk = Vec::new();
    let nd = 6048;
    let erased = |l: usize, fresh: bool| DataCells {
        cells: Arc::from(vec![Complex32::default(); nd]),
        noise: Arc::from(vec![1e6f32; nd]),
        l,
        frame: 0,
        tps_word: None,
        params: tps,
        erased: true,
        fresh,
        time: 0.0,
        rate: Bandwidth::B8.natural_rate(),
    };
    for i in 0..400 {
        let (cells, l) = g.next_cells();
        let d = if (150..162).contains(&i) {
            erased(l, i == 150)
        } else {
            DataCells {
                cells: Arc::from(cells),
                noise: Arc::from(vec![0.001f32; nd]),
                l,
                frame: 0,
                tps_word: None,
                params: tps,
                erased: false,
                fresh: false,
                time: 0.0,
                rate: Bandwidth::B8.natural_rate(),
            }
        };
        fec.push(&d, &mut pk);
    }
    let good_after = pk
        .iter()
        .skip(2400)
        .filter(|p| check_test_packet(&p.0, 0x0100).is_some())
        .count();
    eprintln!(
        "{} packets, {good_after} good after the restart, phase {:?}",
        pk.len(),
        fec.inner.stats.phase
    );
    assert_eq!(fec.inner.stats.phase, Some(0));
    assert!(good_after > 2000);
}
