//! The stated start of each burst: on a live radio whose host times bursts, the first sample of each frame that
//! `lora.modulate` writes carries its time on air, and that sample is the first of the frame.

use crate::common::{dial, invoke, receiver};
use sdrkit_core::{Clock, Complex32, ConfigError, InputInfo, Output, Receiver, Value};
use sdrkit_graph::components::{HostData, HostPort, HostStreams, TxSink};
use sdrkit_graph::{
    Component, ComponentInfo, Format, Graph, GraphBuilder, Io, Kind, Port, RateFrom, Timed,
};
use sdrkit_proto_lora_phy::LoraParams;
use sdrkit_stack_lora::components::modulate::LoraModulate;
use sdrkit_stack_lora::mesh::types::{AfskTx, TxKind, TxRequest};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// The clock of the host.
const NOW: f64 = 1_800_000_000.0;
/// The lead that the host asks for.
const LEAD: Duration = Duration::from_millis(300);

/// The host's transmit stream with a lead: all samples in order, the stated starts (place, UTC) and the ends (the
/// place after the last sample of a burst).
#[derive(Default)]
struct TimedHost {
    samples: Mutex<Vec<Complex32>>,
    starts: Mutex<Vec<(usize, f64)>>,
    ends: Mutex<Vec<usize>>,
}

impl HostStreams for TimedHost {
    fn write(&self, port: HostPort, data: HostData<'_>) {
        if let (
            HostPort::Transmit,
            HostData::Iq {
                samples,
                end,
                start,
                ..
            },
        ) = (port, data)
        {
            let mut all = self.samples.lock().unwrap();
            if let Some(utc) = start {
                self.starts.lock().unwrap().push((all.len(), utc));
            }
            all.extend_from_slice(samples);
            if end {
                self.ends.lock().unwrap().push(all.len());
            }
        }
    }
    fn transmit_lead(&self) -> Duration {
        LEAD
    }
}

impl TimedHost {
    /// Checks that each burst states its start on its first sample, which is not zero, and returns the starts.
    fn checked_starts(&self) -> Vec<(usize, f64)> {
        let all = self.samples.lock().unwrap();
        let starts = self.starts.lock().unwrap().clone();
        let ends = self.ends.lock().unwrap().clone();
        assert_eq!(starts.len(), ends.len(), "a start for each burst");
        for (k, (at, _)) in starts.iter().enumerate() {
            let begin = if k == 0 { 0 } else { ends[k - 1] };
            assert_eq!(*at, begin, "the start is on the first sample of burst {k}");
            assert!(
                all[*at].norm() > 0.0,
                "burst {k} begins with its first sample"
            );
        }
        starts
    }
}

/// Gives the graph of `rx` the host stream `h` and the clock [`NOW`].
fn connect(rx: &mut dyn Receiver, h: &Arc<TimedHost>) {
    let g = rx
        .as_any_mut()
        .and_then(|a| a.downcast_mut::<Graph>())
        .expect("a graph");
    let mut ctx = g.context().clone();
    ctx.streams = Some(Arc::clone(h) as Arc<dyn HostStreams>);
    ctx.clock = Clock::fixed(NOW);
    g.set_context(ctx);
}

#[test]
fn a_raw_frame_on_a_live_radio_states_its_start_on_its_first_sample() {
    const RATE: f64 = 1e6;
    const STEP: usize = 50_000;
    let d = &sdrkit_stack_lora::LORARAW;
    let dial = dial(d);
    let mut a = receiver(d, None, &[]);
    a.set_transmit(true);
    a.configure(&InputInfo::new(RATE, dial + 200e3, dial))
        .unwrap();
    let h = Arc::new(TimedHost::default());
    connect(a.as_mut(), &h);
    let silence = vec![Complex32::default(); STEP];
    let mut out = Output::default();
    a.feed(&silence, &mut out);
    invoke(
        a.as_mut(),
        "send",
        &[("payload", Value::Bytes(b"sent by sdrkit"[..].into()))],
    )
    .unwrap();
    for _ in 0..20 {
        out.clear();
        a.feed(&silence, &mut out);
    }
    let starts = h.checked_starts();
    assert_eq!(starts.len(), 1, "one burst");
    let utc = starts[0].1;
    let lead = NOW + LEAD.as_secs_f64();
    assert!(
        utc >= lead && utc <= lead + 2.0 * STEP as f64 / RATE + 1e-6,
        "the start is the lead and at most two passes after the clock: {}",
        utc - NOW
    );
}

/// Puts out the frames it was given on its first pass.
struct Frames(Vec<TxRequest>);

impl Component for Frames {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "test.frames",
            vec![Port::iq("in")],
            vec![Port::typed::<TxRequest>("out").rate_from(RateFrom::Fixed(0.0))],
        )
    }
    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        Ok(vec![Format::new(Kind::typed::<TxRequest>(), 0.0)])
    }
    fn process(&mut self, io: &mut Io) {
        for r in self.0.drain(..) {
            io.items_out::<TxRequest>(0).push(Timed {
                time: 0.0,
                value: r,
            });
        }
    }
    fn values(&self) -> sdrkit_core::Values {
        sdrkit_graph::no_values()
    }
}

#[test]
fn frames_written_in_one_pass_each_state_a_start_after_the_one_before() {
    const RATE: f64 = 250e3;
    const CENTRE: f64 = 433.775e6;
    let request = |id, lora, afsk| TxRequest {
        id,
        payload: b"two frames in one pass"[..].to_vec(),
        lora,
        afsk,
        freq_hz: CENTRE,
        not_before: 0.0,
        cw: 3,
        what: TxKind::Raw,
        message_key: None,
    };
    let frames = vec![
        request(1, Some(LoraParams::new(7, 125e3, 5, 8, 0x12)), None),
        request(2, None, Some(AfskTx::default())),
    ];
    let mut g = GraphBuilder::new()
        .add("frames", Frames(frames))
        .add("modulate", LoraModulate::default())
        .add("tx", TxSink::default())
        .input("frames")
        .input("modulate.in")
        .connect("frames.out", "modulate.frames")
        .connect("modulate.out", "tx.in")
        .build()
        .expect("the graph");
    g.configure(&InputInfo::new(RATE, CENTRE, CENTRE)).unwrap();
    let h = Arc::new(TimedHost::default());
    connect(&mut g, &h);
    let mut out = Output::default();
    g.feed(&vec![Complex32::default(); RATE as usize], &mut out);
    let starts = h.checked_starts();
    assert_eq!(starts.len(), 2, "two bursts");
    assert!(starts[0].1 >= NOW + LEAD.as_secs_f64());
    let first_s = (starts[1].0 - starts[0].0) as f64 / RATE;
    assert!(
        (starts[1].1 - (starts[0].1 + first_s)).abs() < 1e-9,
        "the second frame goes on air when the first ends"
    );
}

/// The samples of the modulator for fixed frames, in passes of 0.1 s: two LoRa frames, AFSK 1200 with and without
/// pre-emphasis and G3RUH 9600, the first four in one pass; FNV-1a over the bits of every sample, the stated starts
/// (place, UTC to 1 µs) and the ends.
#[test]
fn the_samples_of_the_modulator_keep_their_digest() {
    const RATE: f64 = 250e3;
    const CENTRE: f64 = 433.775e6;
    let request = |id, lora, afsk| TxRequest {
        id,
        payload: b"frames for the digest"[..].to_vec(),
        lora,
        afsk,
        freq_hz: CENTRE,
        not_before: 0.0,
        cw: 3,
        what: TxKind::Raw,
        message_key: None,
    };
    let afsk = |baud, pre_emphasis| AfskTx {
        baud,
        pre_emphasis,
        ..AfskTx::default()
    };
    let frames = vec![
        request(1, Some(LoraParams::new(7, 125e3, 5, 8, 0x12)), None),
        request(2, None, Some(afsk(1200, false))),
        request(3, None, Some(afsk(1200, true))),
        request(4, None, Some(afsk(9600, false))),
        request(5, Some(LoraParams::new(9, 62.5e3, 8, 12, 0x2b)), None),
    ];
    let mut g = GraphBuilder::new()
        .add("frames", Frames(frames))
        .add("modulate", LoraModulate::default())
        .add("tx", TxSink::default())
        .input("frames")
        .input("modulate.in")
        .connect("frames.out", "modulate.frames")
        .connect("modulate.out", "tx.in")
        .build()
        .expect("the graph");
    g.configure(&InputInfo::new(RATE, CENTRE, CENTRE)).unwrap();
    let h = Arc::new(TimedHost::default());
    connect(&mut g, &h);
    let mut out = Output::default();
    for _ in 0..30 {
        out.clear();
        g.feed(&vec![Complex32::default(); (RATE / 10.0) as usize], &mut out);
    }
    let starts = h.checked_starts();
    assert_eq!(starts.len(), 5, "five bursts");
    let mut d = 0xcbf2_9ce4_8422_2325u64;
    let mut put = |v: u64| {
        for b in v.to_le_bytes() {
            d = (d ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3);
        }
    };
    for z in h.samples.lock().unwrap().iter() {
        put(u64::from(z.re.to_bits()) << 32 | u64::from(z.im.to_bits()));
    }
    for (at, utc) in starts {
        put(at as u64);
        put((utc * 1e6).round() as u64);
    }
    for e in h.ends.lock().unwrap().iter() {
        put(*e as u64);
    }
    eprintln!("lora transmit digest {d:#x}");
    assert_eq!(d, 0x6c3fc6f7e175aab6);
}
