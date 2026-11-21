//! The busy state per frame: each detector reports when it finds a preamble and when its frame is over, with the input sample,
//! so a transmitter can wait for a free channel at the time of each frame.
//!
//! A detector confirms a preamble after three matching windows of one symbol, so the channel turns busy between 2 and 5 symbols
//! after the first up-chirp. It turns free at the frame's end (`TxFrame::end_s`, from the Semtech time on air).

use crate::common::AMP;
use sdrkit_core::Complex32;
use sdrkit_graph::{Buffer, Component, Data, Format, Io, Kind};
use sdrkit_proto_lora_phy::bank::{Bank, BankChannel, BankPlan};
use sdrkit_proto_lora_phy::components::{BankState, LoraBank};
use sdrkit_proto_lora_phy::{BusyChange, ChannelRx, DetectorConfig, LoraParams, TxFrame};

const RATE: f64 = 500e3;

fn params() -> LoraParams {
    LoraParams::new(8, 125e3, 5, 8, 0x12)
}

/// Two frames at 20 ms and 120 ms, 30 kHz above the centre.
fn two_frames(p: &LoraParams) -> (Vec<Complex32>, [TxFrame; 2]) {
    let a = TxFrame::new(p, b"first frame")
        .unwrap()
        .at(0.02)
        .carrier(30e3)
        .amplitude(AMP);
    let b = TxFrame::new(p, &[9u8; 30])
        .unwrap()
        .at(0.12)
        .carrier(30e3)
        .amplitude(AMP);
    let mut x = vec![Complex32::default(); (0.25 * RATE) as usize];
    a.add_into(&mut x, RATE, 0);
    b.add_into(&mut x, RATE, 0);
    (x, [a, b])
}

fn channel_changes(x: &[Complex32], piece: usize) -> Vec<BusyChange> {
    let mut rx =
        ChannelRx::new(RATE, 30e3, 125e3, vec![DetectorConfig::new(8, 125e3, 0x12)]).unwrap();
    let mut out = Vec::new();
    let mut changes = Vec::new();
    for c in x.chunks(piece) {
        rx.process(c, &mut out);
        changes.extend_from_slice(rx.busy_changes());
    }
    assert_eq!(out.len(), 2, "both frames decode");
    changes
}

fn check_brackets(changes: &[BusyChange], frames: &[TxFrame; 2], p: &LoraParams) {
    let sym = p.symbol_s();
    assert_eq!(changes.len(), 4, "{changes:?}");
    for (k, f) in frames.iter().enumerate() {
        let (on, off) = (&changes[2 * k], &changes[2 * k + 1]);
        assert!(on.busy && !off.busy, "{changes:?}");
        let t_on = on.at as f64 / RATE - f.start_s;
        let t_off = off.at as f64 / RATE - f.end_s();
        assert!(
            t_on >= 2.0 * sym && t_on <= 5.0 * sym,
            "frame {k}: busy {:.2} symbols after its start",
            t_on / sym
        );
        assert!(
            t_off.abs() <= 0.05 * sym,
            "frame {k}: free {:.3} symbols from its end",
            t_off / sym
        );
    }
}

#[test]
fn a_channel_turns_busy_at_each_preamble_and_free_at_each_frame_end() {
    let p = params();
    let (x, frames) = two_frames(&p);
    // In one piece: both frames, four changes, in order.
    let whole = channel_changes(&x, x.len());
    check_brackets(&whole, &frames, &p);
    // The changes do not depend on how the input is split.
    for piece in [1000, 4099] {
        assert_eq!(channel_changes(&x, piece), whole, "piece {piece}");
    }
}

#[test]
fn a_bank_names_each_change_by_its_place_in_the_states() {
    let p = params();
    let (x, frames) = two_frames(&p);
    let centre = 868.0e6;
    // The first channel lies out of band, so the detector of the second channel is the second state.
    let plan = BankPlan {
        channels: vec![
            BankChannel {
                freq_hz: centre + 5e6,
                bw: 125e3,
                detectors: vec![DetectorConfig::new(8, 125e3, 0x12)],
            },
            BankChannel {
                freq_hz: centre + 30e3,
                bw: 125e3,
                detectors: vec![DetectorConfig::new(8, 125e3, 0x12)],
            },
        ],
        fft_from: 0,
    };
    let base = 1_000_000;
    let mut b = Bank::new(RATE, centre, plan, base).unwrap();
    let mut out = Vec::new();
    let mut changes = Vec::new();
    for c in x.chunks(2048) {
        b.process(c, &mut out);
        changes.extend_from_slice(b.busy_changes());
    }
    assert_eq!(out.len(), 2);
    assert!(changes
        .iter()
        .all(|c| c.detector == 1 && c.channel_hz == centre + 30e3));
    let local: Vec<BusyChange> = changes
        .iter()
        .map(|c| BusyChange {
            at: c.at - base,
            ..*c
        })
        .collect();
    check_brackets(&local, &frames, &p);
    assert!(!b.busy() && !b.states()[1].busy);
}

#[test]
fn the_component_returns_a_state_at_each_busy_change() {
    let p = params();
    let (x, frames) = two_frames(&p);
    let plan = BankPlan {
        channels: vec![BankChannel {
            freq_hz: 30e3,
            bw: 125e3,
            detectors: vec![DetectorConfig::new(8, 125e3, 0x12)],
        }],
        fft_from: 0,
    };
    let mut c = LoraBank::new(plan);
    let fin = Format::iq(RATE, 0.0, 0.0);
    let outs = c.configure(std::slice::from_ref(&fin)).unwrap();
    let mut input = Buffer::new(&Kind::Iq);
    input.data = Data::Iq(x.clone());
    let inputs = [input];
    let formats = [fin, outs[0].clone(), outs[1].clone(), outs[2].clone()];
    let mut bufs = [
        Buffer::new(&outs[0].kind),
        Buffer::new(&outs[1].kind),
        Buffer::new(&outs[2].kind),
    ];
    {
        let mut io = Io::standalone(&inputs, &formats, &[0], &[1, 2, 3], &mut bufs);
        io.set_tick(true);
        c.process(&mut io);
    }
    let states: Vec<BankState> = match &bufs[2].data {
        Data::Items(Some(b)) => b
            .as_any()
            .downcast_ref::<Vec<sdrkit_graph::Timed<BankState>>>()
            .unwrap()
            .iter()
            .map(|t| t.value.clone())
            .collect(),
        _ => Vec::new(),
    };
    // Four changes inside the one piece, then the tick at its end.
    let busy: Vec<bool> = states.iter().map(|s| s.busy).collect();
    assert_eq!(busy, [true, false, true, false, false]);
    assert!(states.windows(2).all(|w| w[0].time <= w[1].time));
    let sym = p.symbol_s();
    assert!((states[1].time - frames[0].end_s()).abs() <= 0.25 * sym);
    assert!((states[3].time - frames[1].end_s()).abs() <= 0.25 * sym);
    assert!(states[0].detectors[0].busy && !states[1].detectors[0].busy);
    assert_eq!(states[4].time, x.len() as f64 / RATE);
}

#[test]
fn a_frame_leaves_the_graph_as_an_event() {
    use sdrkit_core::publish::keys;
    use sdrkit_graph::Item;
    use sdrkit_proto_lora_phy::components::{LoraFrameFields, FRAME_EVENT};
    use sdrkit_proto_lora_phy::LoraFrame;
    let f = LoraFrame {
        payload: b"hi".to_vec(),
        sf: 8,
        snr_db: 3.5,
        ..LoraFrame::default()
    };
    let mut e = FRAME_EVENT.event(1.0, 868.1e6);
    f.write_fields(&mut e.fields);
    assert_eq!(e.at(&LoraFrameFields::PAYLOAD), Some(&b"hi"[..]));
    assert_eq!(e.at(&LoraFrameFields::SF), Some(8));
    assert_eq!(e.get(&keys::SNR_DB), Some(3.5));
}
