//! The cell stacks at the sample rates that common radios offer, and a change of rate while a stack runs: no component
//! stops, and the cells of the test signal are found again after the change.

use crate::common::*;
use sdrkit_core::{Complex32, GenConfig, InputInfo, Output, Receiver, StackDescriptor, Values};
use sdrkit_graph::StackGraph;
use sdrkit_stack_cell::nr::gen::{scene, NrGenOptions, NrGenerator};
use sdrkit_stack_cell::{CELL, GSMR, NBIOT, NR, UMTS};

/// The rates that the common radios offer, Hz.
const RATES: [f64; 9] = [2.56e6, 3.2e6, 4e6, 5e6, 8e6, 10e6, 12.5e6, 16e6, 20e6];

/// The NR test scene `n` at `rate`, and the radio centre.
fn nr_signal(n: u32, rate: f64) -> (f64, Vec<Complex32>) {
    let (centre, cells) = scene(n);
    let x = impaired(NrGenerator::new(rate, cells, NrGenOptions::default()), &snr(15.0), 0.08);
    (centre, x)
}

/// The LTE test scene 0 (PCI 123) at `rate`, and the radio centre.
fn lte_signal(rate: f64) -> (f64, Vec<Complex32>) {
    let c = sdrkit_stack_cell::lte::gen::scene(0, None);
    let centre = sdrkit_stack_cell::bands::earfcn_to_mhz(c.earfcn) * 1e6;
    let g = sdrkit_stack_cell::lte::gen::LteGenerator::new(rate, c, Default::default());
    (centre, impaired(g, &snr(25.0), 0.3))
}

/// Configures `rx` for `input`, as the engine does when the radio's rate changes, and feeds `x` in radio-sized pieces.
fn feed(rx: &mut StackGraph, input: InputInfo, x: &[Complex32]) -> Result<(), String> {
    rx.configure(&input).map_err(|e| e.to_string())?;
    let mut out = Output::default();
    for p in x.chunks(65536) {
        rx.feed(p, &mut out);
        out.clear();
    }
    Ok(())
}

/// The PCIs that `rx` lists, after waiting for its searches.
fn pcis(rx: &mut StackGraph) -> Vec<i64> {
    rx.finish(&mut Output::default());
    cells(&state(rx)).iter().filter_map(|c| c["pci"].as_i64()).collect()
}

#[test]
fn nr_at_every_rate() {
    for rate in RATES {
        // Scene 1: a 15 kHz block (3.6 MHz). Scene 2: a 30 kHz block (7.2 MHz).
        for (n, pci, least) in [(1u32, 222i64, 3.84e6), (2, 17, 7.68e6)] {
            let (centre, x) = nr_signal(n, rate);
            let mut rx = stack_graph(&NR, &[]);
            let r = feed(&mut rx, InputInfo::new(rate, centre, centre).recording(), &x);
            if rate < NR.tuning.min_rate_hz {
                let e = r.expect_err("a rate below the least is refused");
                assert!(e.contains("3840000"), "{e}");
                continue;
            }
            r.unwrap_or_else(|e| panic!("{rate}: {e}"));
            let found = pcis(&mut rx);
            if rate >= least {
                assert_eq!(found, [pci], "scene {n} at {rate}");
            }
        }
    }
}

#[test]
fn lte_at_every_rate() {
    for rate in RATES {
        let (centre, x) = lte_signal(rate);
        let mut rx = stack_graph(&CELL, &[]);
        feed(&mut rx, InputInfo::new(rate, centre, centre).recording(), &x)
            .unwrap_or_else(|e| panic!("{rate}: {e}"));
        assert_eq!(pcis(&mut rx), [123], "{rate}");
    }
}

/// A live input runs a search on a helper thread. A change of rate while a search is out waits for it, and the stack goes on
/// at the new rate.
#[test]
fn nr_and_lte_go_on_after_a_rate_change() {
    for live in [true, false] {
        let input = |rate, centre| {
            let i = InputInfo::new(rate, centre, centre);
            if live {
                i
            } else {
                i.recording()
            }
        };
        let mut rx = stack_graph(&NR, &[]);
        for rate in [20e6, 16e6, 8e6, 12.5e6] {
            let (centre, x) = nr_signal(2, rate);
            clear(&mut rx);
            feed(&mut rx, input(rate, centre), &x).unwrap_or_else(|e| panic!("NR at {rate}, live {live}: {e}"));
            assert_eq!(pcis(&mut rx), [17], "NR at {rate}, live {live}");
        }
        let mut rx = stack_graph(&CELL, &[]);
        for rate in [20e6, 16e6, 2.56e6, 10e6] {
            let (centre, x) = lte_signal(rate);
            clear(&mut rx);
            feed(&mut rx, input(rate, centre), &x).unwrap_or_else(|e| panic!("LTE at {rate}, live {live}: {e}"));
            assert_eq!(pcis(&mut rx), [123], "LTE at {rate}, live {live}");
        }
    }
}

/// The other search stacks of the family take a change of rate while a live search runs.
#[test]
fn other_cell_stacks_take_a_rate_change() {
    let stacks: [&StackDescriptor; 3] = [&NBIOT, &GSMR, &UMTS];
    for desc in stacks {
        let mut rx = stack_graph(desc, &[]);
        let centre = start_hz(desc);
        for rate in [desc.tuning.rate_hz, 2.0 * desc.tuning.rate_hz, desc.tuning.min_rate_hz] {
            let gcfg = GenConfig::new(rate.into(), Values::defaults(&(desc.settings)()));
            let make = desc.make_generator.expect("a generator");
            let mut g = make(&gcfg).expect("the generator");
            let mut x = vec![Complex32::default(); (rate * 0.3) as usize];
            let n = g.generate(&mut x);
            x.truncate(n);
            feed(&mut rx, InputInfo::new(rate, centre, centre), &x)
                .unwrap_or_else(|e| panic!("{} at {rate}: {e}", desc.id));
            rx.finish(&mut Output::default());
        }
    }
}
