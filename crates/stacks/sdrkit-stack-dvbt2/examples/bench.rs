//! Runs the benchmarks of the DVB-T2 crate and prints their results, and for each sender the time of its start and the
//! part of a frame that takes longest to make.

use sdrkit_proto_mpegts::sendqueue::SendQueue;
use sdrkit_stack_dvbt2::transmit::{bench_default, bench_narrow, bench_sender, Chain};

fn main() {
    for b in sdrkit_stack_dvbt2::benchmarks() {
        let r = (b.run)(3.0);
        println!("{}: {r:?}", b.name);
    }
    for (name, cfg, frames) in [("default", bench_default(), 2), ("narrow", bench_narrow(), 4)] {
        // The start of a transmission: the transmitter is built and the first frame is coded at once.
        let t = std::time::Instant::now();
        let mut chain = Chain::new(&cfg).expect("a chain");
        let built_s = t.elapsed().as_secs_f64();
        let mut queue = SendQueue::new(chain.plan().bit_rate);
        let t = std::time::Instant::now();
        chain.part(&mut queue, &mut Vec::new());
        println!(
            "sender {name}: start takes {:.1} ms to build the transmitter and {:.1} ms to code the first frame",
            built_s * 1e3,
            t.elapsed().as_secs_f64() * 1e3
        );
        let (_, worst) = bench_sender(3.0, &cfg, frames);
        println!(
            "sender {name}: the slowest part took {:.3} ms for {:.3} ms of signal",
            worst.took_s * 1e3,
            worst.signal_s * 1e3
        );
    }
}
