//! The benchmarks of the analogue video stacks: each runs a stack's receiver on its own test signal at the stack's rate, or the
//! modulator of a sender as it sends.

use crate::gen_atv::{AtvGenParams, AtvGenerator};
use sdrkit_core::bench::measure;
use sdrkit_core::{
    BenchResult, Benchmark, Complex32, Generator, InputInfo, Output, StackConfig, Values,
};

fn signal(g: &mut dyn Generator, n: usize) -> Vec<Complex32> {
    let mut x = vec![Complex32::default(); n];
    g.generate(&mut x);
    x
}

fn run_rx(
    desc: &sdrkit_core::StackDescriptor,
    rate: f64,
    x: &[Complex32],
    seconds: f64,
    warm: usize,
) -> BenchResult {
    let values = Values::defaults(&(desc.settings)());
    let Ok(mut rx) = (desc.make_receiver)(&StackConfig::new(values)) else {
        return BenchResult::default();
    };
    rx.configure(&InputInfo::new(rate, 600e6, 600e6).recording())
        .ok();
    let mut out = Output::default();
    // Warm up to lock.
    for c in x[..warm].chunks(65_536) {
        out.clear();
        rx.feed(c, &mut out);
    }
    let rest = &x[warm..];
    measure(seconds, rest.len(), rest.len() as f64 / rate, || {
        for c in rest.chunks(65_536) {
            out.clear();
            rx.feed(c, &mut out);
        }
    })
}

fn atv_am(seconds: f64) -> BenchResult {
    let rate = 10e6;
    let mut g = AtvGenerator::new(AtvGenParams {
        rate_hz: rate,
        ..Default::default()
    });
    let mut x = signal(&mut g, (1.2 * rate) as usize);
    // 40 dB carrier to noise: the peak carrier power 0.16 against the noise in 5 MHz.
    if let Ok(mut n) = sdrkit_algo_sim::noise::Awgn::with_power(0.16e-4 * rate / 5e6, 1) {
        n.process(&mut x);
    }
    run_rx(&crate::ATV, rate, &x, seconds, (0.8 * rate) as usize)
}

/// The `atv` sender at 10 Msps, PAL B/G with sound: a new picture of the host every field (the card and the same card
/// mirrored, given as BGRA at the picture size and fitted), the host's audio, and the samples of each field.
fn atv_tx(seconds: f64) -> BenchResult {
    use crate::transmit::{fit_frame, AtvTx, TvModulator};
    use crate::typed::TvPixels;
    let rate = 10e6;
    let mut g = AtvGenerator::new(AtvTx::default().params(rate));
    let card = g.card().clone();
    let (w, h) = (card.w, card.h);
    let bgra = |mirror: bool| -> Vec<u8> {
        let mut b = Vec::with_capacity(w * h * 4);
        for r in 0..h {
            for c in 0..w {
                let [red, green, blue] = card.rgb(if mirror { w - 1 - c } else { c }, r);
                let q = |x: f64| (x.clamp(0.0, 1.0) * 255.0).round() as u8;
                b.extend_from_slice(&[q(blue), q(green), q(red), 255]);
            }
        }
        b
    };
    let frames = [bgra(false), bgra(true)];
    let field = (rate / 50.0) as usize;
    let mut x = vec![Complex32::default(); field];
    let audio = vec![0.0f32; g.programme_due(field) + 1];
    measure(seconds, 2 * field, 0.04, || {
        for f in &frames {
            if let Ok(p) = fit_frame(f, w as u32, h as u32, TvPixels::Bgra, w, h) {
                TvModulator::show(&mut g, Some(p));
            }
            let due = g.programme_due(field);
            g.push_programme(&audio[..due]);
            g.generate(&mut x);
        }
    })
}

/// The `fpv` receiver at 20 Msps on its PAL test card with the 6.5 MHz sound, at 30 dB carrier to noise.
fn fpv_rx(seconds: f64) -> BenchResult {
    let rate = 20e6;
    let mut g = crate::fpv::FpvGenerator::new(crate::fpv::FpvGenParams::default());
    let mut x = signal(&mut g, (1.2 * rate) as usize);
    // The carrier power 0.09 against the noise in 18 MHz.
    if let Ok(mut n) = sdrkit_algo_sim::noise::Awgn::with_power(0.09e-3 * rate / 18e6, 1) {
        n.process(&mut x);
    }
    run_rx(&crate::FPV, rate, &x, seconds, (0.8 * rate) as usize)
}

/// The benchmarks of the `fpv` stack.
pub fn fpv_benchmarks() -> Vec<Benchmark> {
    vec![Benchmark {
        name: "analogtv.fpv_20m",
        run: fpv_rx,
    }]
}

/// The benchmarks of the `atv` stack.
pub fn benchmarks() -> Vec<Benchmark> {
    vec![
        Benchmark {
            name: "analogtv.atv_am_10m",
            run: atv_am,
        },
        Benchmark {
            name: "analogtv.atv_tx_10m",
            run: atv_tx,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Prints the cost of each component and section of a receiver, in cores, as the fastest of `runs` passes over the
    /// part of `x` after `warm`.
    fn split(
        desc: &sdrkit_core::StackDescriptor,
        rate: f64,
        x: &[Complex32],
        warm: usize,
        runs: usize,
    ) {
        let values = Values::defaults(&(desc.settings)());
        let mut rx = (desc.make_receiver)(&StackConfig::new(values)).expect("the receiver builds");
        rx.configure(&InputInfo::new(rate, 600e6, 600e6).recording())
            .ok();
        let mut out = Output::default();
        for c in x[..warm].chunks(65_536) {
            out.clear();
            rx.feed(c, &mut out);
        }
        let rest = &x[warm..];
        let secs = rest.len() as f64 / rate;
        let mut best: Vec<(String, &'static str, u64, usize)> = Vec::new();
        let mut total = f64::INFINITY;
        let mut loads = Vec::new();
        let read = |rx: &mut Box<dyn sdrkit_core::Receiver>,
                    l: &mut Vec<sdrkit_core::ComponentLoad>| {
            if let Some(s) = rx.staged() {
                s.component_loads(l);
            }
        };
        for _ in 0..runs {
            read(&mut rx, &mut loads);
            let before: Vec<u64> = loads.iter().map(|l| l.busy_ns).collect();
            let t0 = std::time::Instant::now();
            for c in rest.chunks(65_536) {
                out.clear();
                rx.feed(c, &mut out);
            }
            total = total.min(t0.elapsed().as_secs_f64());
            read(&mut rx, &mut loads);
            if best.is_empty() {
                best = loads
                    .iter()
                    .map(|l| (l.component.clone(), l.section, u64::MAX, l.stage))
                    .collect();
            }
            for (i, l) in loads.iter().enumerate() {
                let d = l.busy_ns - before.get(i).copied().unwrap_or(0);
                if let Some(b) = best.get_mut(i) {
                    b.2 = b.2.min(d);
                }
            }
        }
        println!("{}: total {:.3} of a core", desc.id, total / secs);
        let mut stages = [0.0f64; 4];
        for (c, s, ns, stage) in best {
            let v = ns as f64 * 1e-9 / secs;
            if s.is_empty() {
                stages[stage.min(3)] += v;
            }
            if v >= 0.002 {
                println!("  {c:12} {s:12} {v:.3}  stage {stage}");
            }
        }
        println!("  stages {stages:.3?}");
    }

    #[test]
    #[ignore = "a measurement; run it in a release build"]
    fn cost_split() {
        let rate = 20e6;
        let mut g = crate::fpv::FpvGenerator::new(crate::fpv::FpvGenParams::default());
        let mut x = signal(&mut g, (1.2 * rate) as usize);
        if let Ok(mut n) = sdrkit_algo_sim::noise::Awgn::with_power(0.09e-3 * rate / 18e6, 1) {
            n.process(&mut x);
        }
        split(&crate::FPV, rate, &x, (0.8 * rate) as usize, 6);
        let rate = 10e6;
        let mut g = AtvGenerator::new(AtvGenParams {
            rate_hz: rate,
            ..Default::default()
        });
        let mut x = signal(&mut g, (1.2 * rate) as usize);
        if let Ok(mut n) = sdrkit_algo_sim::noise::Awgn::with_power(0.16e-4 * rate / 5e6, 1) {
            n.process(&mut x);
        }
        split(&crate::ATV, rate, &x, (0.8 * rate) as usize, 6);
    }
}
