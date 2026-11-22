//! Helpers shared by the tests: a transmitter of a given shape, and a receiver run that collects transport packets.

use sdrkit_core::Complex32;
use sdrkit_proto_ipcast::adapt::UpDeframer;
use sdrkit_proto_mpegts::source::{Fill, Numbered, NumberedCheck, NumberedSource};
use sdrkit_stack_dvbt2::bicm::Coding;
use sdrkit_stack_dvbt2::params::*;
use sdrkit_stack_dvbt2::pilots::FrameShape;
use sdrkit_stack_dvbt2::rx::{RxSettings, RxStatus, T2Receiver};
use sdrkit_stack_dvbt2::tx::{MisoOut, Path, PlpKind, PlpSpec, TimeIl, Transmitter, TxConfig};

/// The numbered pattern of the data PLP.
pub const PATTERN: Numbered = Numbered::new(0x0101, Fill::I31N7);

/// A single-PLP configuration.
pub fn single(fft: Fft, guard: Guard, pp: PilotPattern, coding: Coding, blocks: u16) -> TxConfig {
    TxConfig {
        bandwidth: Bandwidth::B8,
        shape: FrameShape {
            fft,
            extended: false,
            pp,
            guard,
            miso: false,
            tr: false,
            l_data: 0,
        },
        profile: Profile::Base,
        l1_mod: Modulation::Qpsk,
        num_t2_frames: 2,
        plps: vec![PlpSpec {
            id: 0,
            kind: PlpKind::Type1,
            group: 0,
            coding,
            blocks,
            time_il: TimeIl::Blocks(1),
            source: Box::new(NumberedSource::new(PATTERN)),
        }],
        sub_slices: 1,
        cell_id: 0x1234,
        network_id: 0x3001,
        t2_system_id: 0x8001,
        repetition: false,
        post_scrambled: false,
        miso: MisoOut::Sum([
            Path {
                gain: Complex32::new(1.0, 0.0),
                delay: 0,
            },
            Path {
                gain: Complex32::new(0.6, 0.3),
                delay: 0,
            },
        ]),
        fef: None,
    }
}

/// Samples of `frames` T2-frames from `tx`, with `lead` zero samples first.
pub fn signal(tx: &mut Transmitter, frames: usize, lead: usize) -> Vec<Complex32> {
    let mut v = vec![Complex32::default(); lead];
    for _ in 0..frames {
        v.extend(tx.frame().expect("frame"));
    }
    v
}

/// Runs the receiver over `x` in pieces of `chunk` and counts the numbered packets of PLP `plp`.
pub fn receive(
    x: &[Complex32],
    set: RxSettings,
    chunk: usize,
    plp: u8,
) -> (RxStatus, NumberedCheck, usize) {
    let mut rx = T2Receiver::new(set);
    let mut df = UpDeframer::new();
    let mut check = NumberedCheck::default();
    let mut frames = 0usize;
    for c in x.chunks(chunk) {
        rx.feed(c, &mut |b| {
            if b.plp != plp {
                return;
            }
            frames += 1;
            let _ = df.push_frame(b.bytes, &mut |u| check.push(&PATTERN, u.bytes, 0.0));
        });
    }
    (rx.status().clone(), check, frames)
}

/// A plain coding.
pub fn coding(frame: FecFrame, rate: CodeRate, modulation: Modulation, rotated: bool) -> Coding {
    Coding {
        frame,
        rate,
        modulation,
        rotated,
    }
}
