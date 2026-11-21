//! The sender of `dvbt2`: the stack's own receiver returns the host's transport stream bit for bit (null packets and PCR
//! fields aside), also when it starts 0.5 s into the transmission; the L1-pre and L1-post fields are the configured ones;
//! the useful bit rate against a calculation by hand; the level, the peaks and the occupied width of the signal; the end
//! at a frame boundary; and the refusals. Nothing goes to a radio.

use sdrkit_components::tx::SOFT_KNEE as KNEE;
use sdrkit_core::interface::ReadFields;
use sdrkit_core::owner::{self, Owner};
use sdrkit_core::{
    Choice, Clock, CommandError, Complex32, InputInfo, Output, Receiver, SettingSet, StackConfig,
    Value, Values,
};
use sdrkit_graph::components::{HostData, HostPort, HostStreams};
use sdrkit_graph::{RunContext, StackGraph};
use sdrkit_proto_mpegts::packet::{parse, NULL_PID, TS_SIZE};
use sdrkit_proto_mpegts::sendqueue::{QueueError, SendQueue, QUEUE_PACKETS};
use sdrkit_proto_mpegts::source::{NumberedSource, PacketSource, TestMux, TestSound};
use sdrkit_stack_dvbt2::bicm::Coding;
use sdrkit_stack_dvbt2::error::{Rule, T2Error};
use sdrkit_stack_dvbt2::params::*;
use sdrkit_stack_dvbt2::rx::{RxSettings, Stage, T2Receiver};
use sdrkit_stack_dvbt2::settings::{L1Modulation, PlpModulation, TxRate};
use sdrkit_stack_dvbt2::transmit::{Chain, TxConfig, TxError, RISE_S, RMS};
use sdrkit_stack_dvbt2::tx::{FramePlan, TimeIl, Transmitter};
use sdrkit_stack_dvbt2::typed::{Dvbt2Fields, Dvbt2TxFields};
use sdrkit_stack_dvbt2::Dvbt2Setting;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// The dial of the transmitting stack.
const DIAL: f64 = 437e6;
/// The PID of the sound of the host's stream.
const SOUND_PID: u16 = 0x0102;

/// One write to the transmit stream: its samples, its end mark and its stated start.
type Write = (Vec<Complex32>, bool, Option<f64>);

/// The host: what the stack writes to the transmit stream, with the end marks.
#[derive(Default)]
struct Host {
    writes: Mutex<Vec<Write>>,
    lead: Duration,
}

impl HostStreams for Host {
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
            self.writes
                .lock()
                .unwrap()
                .push((samples.to_vec(), end, start));
        }
    }
    fn transmit_lead(&self) -> Duration {
        self.lead
    }
}

impl Host {
    /// Every sample written, and the number of end marks.
    fn sent(&self) -> (Vec<Complex32>, usize) {
        let w = self.writes.lock().unwrap();
        let x = w.iter().flat_map(|(s, _, _)| s.iter().copied()).collect();
        (x, w.iter().filter(|(_, e, _)| *e).count())
    }
}

fn choice<C: Choice>(c: C) -> Value {
    Value::Choice(c.index())
}

/// The choice of `C` that `is` accepts.
fn choice_of<C: Choice>(is: impl Fn(C) -> bool) -> Value {
    let i = (0..C::CHOICES.len() as u16)
        .find(|&i| C::from_index(i).is_some_and(&is))
        .expect("a choice");
    Value::Choice(i)
}

/// The configuration of the stack with these settings, by key.
fn config(settings: &[(&str, Value)]) -> StackConfig {
    let schema = Dvbt2Setting::schema();
    let mut v = Values::defaults(&schema);
    for (k, x) in settings {
        let id = schema.id(k).unwrap();
        v.set(id, x).unwrap();
    }
    StackConfig::new(v)
}

/// The values of the `tx_*` settings of `c`.
fn tx_settings(c: &TxConfig) -> Vec<(&'static str, Value)> {
    vec![
        ("tx_bandwidth", choice(c.bandwidth)),
        ("tx_fft", choice(c.fft)),
        ("tx_guard", choice(c.guard)),
        ("tx_pilots", choice(c.pilots)),
        ("tx_extended", Value::Bool(c.extended)),
        (
            "tx_modulation",
            choice_of(|m: PlpModulation| m.modulation() == c.coding.modulation),
        ),
        ("tx_rate", choice_of(|r: TxRate| r.rate() == c.coding.rate)),
        ("tx_rotation", Value::Bool(c.coding.rotated)),
        ("tx_fec_frame", choice(c.coding.frame)),
        ("tx_ti_blocks", Value::I64(i64::from(c.ti_blocks))),
        ("tx_frame_ms", Value::I64(i64::from(c.frame_ms))),
        (
            "tx_l1_mod",
            choice_of(|m: L1Modulation| m.modulation() == c.l1_mod),
        ),
        ("tx_cell_id", Value::I64(i64::from(c.cell_id))),
        ("tx_network_id", Value::I64(i64::from(c.network_id))),
        ("tx_system_id", Value::I64(i64::from(c.system_id))),
    ]
}

/// The `dvbt2` stack with the sender settings of `c` at `rate`, the transmitter on, and its host.
fn station(c: &TxConfig, rate: f64) -> (StackGraph, Arc<Host>) {
    station_with(c, rate, None)
}

/// [`station`]; with `lead`, on a live radio whose host times bursts with that lead.
fn station_with(c: &TxConfig, rate: f64, lead: Option<Duration>) -> (StackGraph, Arc<Host>) {
    // The receiver of the stack takes the channel width of the sender, so the session's rate fits both.
    let mut settings = tx_settings(c);
    settings.push(("bandwidth", choice(c.bandwidth)));
    let mut st =
        sdrkit_stack_dvbt2::receiver(&config(&settings)).expect("the stack takes the settings");
    let input = InputInfo::new(rate, DIAL, DIAL);
    st.configure(&if lead.is_some() {
        input
    } else {
        input.recording()
    })
    .unwrap();
    let host = Arc::new(Host {
        lead: lead.unwrap_or_default(),
        ..Host::default()
    });
    let mut ctx: RunContext = st.graph().context().clone();
    ctx.streams = Some(host.clone());
    ctx.clock = Clock::fixed(1_800_000_000.0);
    st.set_context(ctx);
    assert!(st.transmits());
    st.set_transmit(true);
    (st, host)
}

/// Runs command `key` of `st` with the bytes `packets` as its argument `packets` when given.
fn call(st: &mut StackGraph, key: &str, packets: Option<&[u8]>) -> Result<(), CommandError> {
    let id = st.commands().id(key).unwrap();
    let mut a = st.commands().def(id).unwrap().new_args();
    if let Some(p) = packets {
        let i = a.decl().id("packets").unwrap();
        a.set_value(i, &Value::Bytes(Arc::from(p))).unwrap();
    }
    owner::invoke(st, id, &a).map(|_| ())
}

/// The refusal of the sender inside a command error.
fn refusal(e: &CommandError) -> Option<TxError> {
    match e {
        CommandError::Failed { source } => source.downcast_ref::<TxError>().copied(),
        _ => None,
    }
}

/// The sender's fields without the text of the problem, and whether a problem is stated.
fn tx_fields(st: &StackGraph) -> (Dvbt2TxFields<'static>, bool) {
    let f = Dvbt2TxFields::read(Owner::results(st).fields()).expect("the sender's fields");
    let problem = f.tx_problem.is_some();
    (
        Dvbt2TxFields {
            tx_problem: None,
            ..f
        },
        problem,
    )
}

/// Feeds `seconds` of silence at `rate` in pieces of 0.05 s; before each piece `top_up` may append packets.
fn run(st: &mut StackGraph, rate: f64, seconds: f64, mut top_up: impl FnMut(&mut StackGraph)) {
    let piece = vec![Complex32::default(); (rate * 0.05) as usize];
    let mut out = Output::default();
    for _ in 0..(seconds / 0.05).round() as usize {
        top_up(st);
        out.clear();
        st.feed(&piece, &mut out);
    }
}

/// The packets of the host: the test multiplex at `bit_rate` (tables, numbered data and a sound with its PCR), its null
/// packets left out.
fn host_stream(bit_rate: f64, count: usize) -> Vec<[u8; TS_SIZE]> {
    let sound = TestSound {
        pid: SOUND_PID,
        mux_bit_rate: bit_rate,
    };
    let mut src = TestMux::dvbt_builder(7, "Loop service")
        .sound(sound)
        .build()
        .expect("a multiplex");
    let mut out = Vec::with_capacity(count);
    let mut p = [0u8; TS_SIZE];
    while out.len() < count {
        src.next_packet(&mut p);
        if pid(&p) != NULL_PID {
            out.push(p);
        }
    }
    out
}

fn pid(p: &[u8]) -> u16 {
    (u16::from(p[1] & 0x1F) << 8) | u16::from(p[2])
}

/// `p` with its PCR field set to zero when it has one.
fn masked(p: &[u8; TS_SIZE]) -> [u8; TS_SIZE] {
    let mut q = *p;
    if parse(p)
        .and_then(|x| x.adaptation)
        .and_then(|a| a.pcr)
        .is_some()
    {
        q[6..12].fill(0);
    }
    q
}

/// The signal to noise ratio of the receiving radio, dB.
const SNR_DB: f64 = 40.0;

/// What the receiver of the stack made of a signal.
struct Received {
    /// The packets that are not null packets and carry no transport_error_indicator.
    packets: Vec<[u8; TS_SIZE]>,
    /// Every packet.
    all: usize,
    /// The signal time at the first packet, s.
    locked_s: Option<f64>,
    results: sdrkit_core::Results,
}

/// The receiver of the stack on `x` at `rate` with noise at [`SNR_DB`] over the occupied band, in a channel of `c`.
fn receive(x: &[Complex32], rate: f64, c: &TxConfig) -> Received {
    let mut rx = sdrkit_stack_dvbt2::receiver(&config(&[("bandwidth", choice(c.bandwidth))]))
        .expect("the receiver");
    rx.configure(&InputInfo::new(rate, DIAL, DIAL).recording())
        .expect("the rate");
    let mut tap = rx.graph_mut().tap("ts.ts", 1 << 14).expect("a tap");
    let mut buf = sdrkit_graph::Buffer::new(&sdrkit_proto_mpegts::stream::kind());
    let mut packets: Vec<[u8; TS_SIZE]> = Vec::new();
    let mut take = |tap: &mut sdrkit_graph::TapReader, packets: &mut Vec<[u8; TS_SIZE]>| {
        while tap.pop(&mut buf) {
            let sdrkit_graph::Data::Items(Some(v)) = &buf.data else {
                continue;
            };
            type Packets = Vec<sdrkit_graph::Timed<sdrkit_proto_mpegts::stream::TsPacket>>;
            for p in v.as_any().downcast_ref::<Packets>().into_iter().flatten() {
                packets.push(*p.value.bytes());
            }
        }
    };
    let mut x = x.to_vec();
    let p = x.iter().map(|z| f64::from(z.norm_sqr())).sum::<f64>() / x.len() as f64;
    sdrkit_dev_test::Awgn::new(p, SNR_DB, c.occupied_hz(), rate, 3).process(&mut x);
    let mut out = Output::default();
    let (mut fed, mut locked_s) = (0usize, None);
    for piece in x.chunks(16384) {
        rx.feed(piece, &mut out);
        out.clear();
        fed += piece.len();
        take(&mut tap, &mut packets);
        if locked_s.is_none() && !packets.is_empty() {
            locked_s = Some(fed as f64 / rate);
        }
    }
    rx.finish(&mut out);
    take(&mut tap, &mut packets);
    let all = packets.len();
    packets.retain(|p| pid(p) != NULL_PID && p[1] & 0x80 == 0);
    Received {
        packets,
        all,
        locked_s,
        results: Owner::results(&rx).clone(),
    }
}

/// One transmission: the sender of `c` at `rate` with the stream `hp`, `lead_s` of null packets first, the packets fed as
/// the queue has room, then `tail_s` of null packets; the samples sent, the end marks and the fields before `stop`.
fn send(
    c: &TxConfig,
    rate: f64,
    hp: &[[u8; TS_SIZE]],
    lead_s: f64,
    tail_s: f64,
) -> (Vec<Complex32>, usize, Dvbt2TxFields<'static>) {
    let (mut st, host) = station(c, rate);
    call(&mut st, "start", None).expect("starts");
    run(&mut st, rate, lead_s, |_| {});
    let mut at = 0;
    while at < hp.len() {
        run(&mut st, rate, 0.05, |st| {
            let waiting = tx_fields(st).0.tx_queue as usize;
            let k = (QUEUE_PACKETS - waiting).min(hp.len() - at).min(2000);
            if k > 0 {
                let bytes: Vec<u8> = hp[at..at + k].iter().flatten().copied().collect();
                call(st, "append", Some(&bytes)).expect("room in the queue");
                at += k;
            }
        });
    }
    let mut waited = 0.0;
    while tx_fields(&st).0.tx_queue > 0 && waited < 10.0 {
        run(&mut st, rate, 0.05, |_| {});
        waited += 0.05;
    }
    run(&mut st, rate, tail_s, |_| {});
    let fields = tx_fields(&st).0;
    call(&mut st, "stop", None).unwrap();
    run(&mut st, rate, 0.4, |_| {});
    assert!(!tx_fields(&st).0.transmitting, "stop ends the transmission");
    let (x, ends) = host.sent();
    (x, ends, fields)
}

/// The loop of `c` at `rate`: what the receiver returns equals what the host gave, from the start of the transmission
/// and, with `late`, from 0.5 s into it. The L1 values that the receiver states are those of `c`.
fn loop_test(c: &TxConfig, rate: f64, packets: usize, late: bool) {
    let plan = c.plan().expect("a frame");
    let hp = host_stream(plan.bit_rate, packets);
    // The receiver that starts late needs two frames to lock after 0.5 s.
    let lead = if late { 0.5 + 4.0 * plan.frame_s } else { 4.0 * plan.frame_s };
    let lead = (lead / 0.05).ceil() * 0.05;
    let (x, ends, f) = send(c, rate, &hp, lead, 3.0 * plan.frame_s + 0.05);
    assert_eq!(ends, 1, "one end mark");
    assert_eq!(f.tx_packets as usize, hp.len());
    assert!(f.tx_nulls > 0 && f.tx_pcr_restamped > 0, "{f:?}");
    assert!((f.tx_bitrate_bps - plan.bit_rate).abs() < 1e-6);
    let want: Vec<_> = hp.iter().map(masked).collect();
    let starts: &[f64] = if late { &[0.0, 0.5] } else { &[0.0] };
    for &from in starts {
        let r = receive(&x[(from * rate) as usize..], rate, c);
        let got: Vec<_> = r.packets.iter().map(masked).collect();
        let d = Dvbt2Fields::read(r.results.fields()).expect("the receiver's fields");
        eprintln!(
            "{} {} {} {} {:?}, sent at {rate:.0} sps, received from {from} s: first packet after {:.3} s ({:.1} frames of {:.1} ms), SNR {:.1} dB, {} packets",
            c.bandwidth.name(),
            c.fft.name(),
            c.coding.modulation.name(),
            c.coding.rate.name(),
            c.coding.frame,
            r.locked_s.unwrap_or(f64::NAN),
            r.locked_s.unwrap_or(f64::NAN) / plan.frame_s,
            plan.frame_s * 1e3,
            d.snr_db.unwrap_or(f64::NAN),
            r.all
        );
        assert_eq!(got.len(), want.len(), "packets returned from {from} s");
        assert!(got == want, "the packets differ from {from} s");
        assert!(r.locked_s.expect("a first packet") < 4.0 * plan.frame_s);
        assert_eq!(
            (d.fft, d.guard, d.pilot_pattern, d.extended_carriers),
            (
                Some(c.fft),
                Some(c.guard),
                Some(c.pilots),
                Some(c.extended)
            )
        );
        assert_eq!(d.l1_modulation, Some(c.l1_mod));
        assert_eq!(
            (d.cell_id, d.network_id, d.t2_system_id),
            (
                Some(i64::from(c.cell_id)),
                Some(i64::from(c.network_id)),
                Some(i64::from(c.system_id))
            )
        );
        assert_eq!(d.data_symbols, Some(plan.l_data as i64));
        assert!((d.net_rate_bit_s.expect("a net rate") - plan.bit_rate).abs() < 1.0);
        assert!(d.snr_db.expect("an SNR") > 30.0);
        assert_eq!((d.frames_bad, d.l1_errors), (0, 0), "from {from} s");
    }
}

/// The amateur profile: 1.7 MHz, 2K, guard 1/8, PP2, rotated QPSK 1/2, 16 200-bit frames.
fn narrow() -> TxConfig {
    TxConfig {
        cell_id: 0x1001,
        network_id: 0x3085,
        system_id: 0x8001,
        ..sdrkit_stack_dvbt2::transmit::bench_narrow()
    }
}

#[test]
fn loop_2k_qpsk_half_at_1700_khz_resampled() {
    loop_test(&narrow(), 2e6, 900, true);
}

#[test]
fn loop_2k_16qam_three_quarters_at_1700_khz() {
    let c = TxConfig {
        coding: Coding {
            frame: FecFrame::Short,
            rate: CodeRate::R3_4,
            modulation: Modulation::Qam16,
            rotated: true,
        },
        guard: Guard::G1_16,
        pilots: PilotPattern::Pp4,
        ..narrow()
    };
    loop_test(&c, c.bandwidth.rate(), 1500, false);
}

#[test]
fn loop_8k_64qam_two_thirds_at_8_mhz_with_the_defaults() {
    let c = TxConfig::default();
    loop_test(&c, c.bandwidth.rate(), 6000, true);
}

#[test]
fn loop_4k_16qam_three_fifths_plain_without_time_interleaving_at_6_mhz() {
    let c = TxConfig {
        bandwidth: Bandwidth::B6,
        fft: Fft::K4,
        guard: Guard::G1_16,
        pilots: PilotPattern::Pp4,
        extended: false,
        coding: Coding {
            frame: FecFrame::Short,
            rate: CodeRate::R3_5,
            modulation: Modulation::Qam16,
            rotated: false,
        },
        ti_blocks: 0,
        frame_ms: 60,
        l1_mod: Modulation::Bpsk,
        cell_id: 7,
        network_id: 8,
        system_id: 9,
    };
    loop_test(&c, c.bandwidth.rate(), 3000, false);
}

#[test]
fn loop_8k_extended_256qam_five_sixths_with_a_closing_symbol_at_7_mhz() {
    let c = TxConfig {
        bandwidth: Bandwidth::B7,
        fft: Fft::K8,
        guard: Guard::G1_4,
        pilots: PilotPattern::Pp1,
        extended: true,
        coding: Coding {
            frame: FecFrame::Normal,
            rate: CodeRate::R5_6,
            modulation: Modulation::Qam256,
            rotated: true,
        },
        ti_blocks: 1,
        frame_ms: 80,
        l1_mod: Modulation::Qam64,
        cell_id: 0,
        network_id: 0,
        system_id: 0,
    };
    assert!(c.shape().has_fc());
    loop_test(&c, c.bandwidth.rate(), 5000, false);
}

/// The samples of `frames` whole T2-frames of the chain of `c` with an empty queue, at the elementary rate.
fn frames_of(c: &TxConfig, frames: usize) -> Vec<Complex32> {
    let mut chain = Chain::new(c).expect("a chain");
    let mut queue = SendQueue::new(chain.plan().bit_rate);
    let (mut x, mut part) = (Vec::new(), Vec::new());
    let n = frames * chain.plan().frame_samples;
    while x.len() < n {
        chain.part(&mut queue, &mut part);
        x.extend_from_slice(&part);
    }
    x.truncate(n);
    x
}

/// The L1-pre and the configurable L1-post of the first PLP that the receiver decodes from `x`.
fn l1_of(x: &[Complex32], bandwidth: Bandwidth) -> (sdrkit_stack_dvbt2::l1::L1Pre, sdrkit_stack_dvbt2::l1::PlpConfig) {
    let mut rx = T2Receiver::new(RxSettings {
        bandwidth,
        ..RxSettings::default()
    });
    for piece in x.chunks(16384) {
        rx.feed(piece, &mut |_| {});
    }
    assert_eq!(rx.status().stage, Stage::Locked);
    assert_eq!(rx.status().l1_errors, 0);
    let pre = rx.status().pre.expect("L1-pre");
    let post = rx.post().expect("L1-post");
    assert_eq!(post.config.plps.len(), 1);
    (pre, post.config.plps[0])
}

#[test]
fn the_l1_pre_fields_of_a_frame_of_60_symbols_are_those_of_the_worked_example() {
    // The defaults with cell id 0x0001, network id 0x3001, system id 0x8001 and L_F = 60: N_P2 = 2 and 58 data symbols.
    let c = TxConfig {
        cell_id: 0x0001,
        network_id: 0x3001,
        system_id: 0x8001,
        ..TxConfig::default()
    };
    let plan = FramePlan {
        l_data: 58,
        blocks: 36,
        ..c.plan().unwrap()
    };
    let mut t: Transmitter = c.transmitter(&plan).expect("a transmitter");
    // T = 7/64 µs: 60 symbols of 8256 T and the P1 symbol of 2048 T last 54.4 ms.
    assert_eq!(t.frame_len(), 60 * 8256 + 2048);
    assert!((t.frame_len() as f64 / t.rate() - 54.4e-3).abs() < 0.05e-3);
    let mut x = Vec::new();
    for _ in 0..3 {
        x.extend(t.frame().unwrap());
    }
    let (pre, plp) = l1_of(&x, c.bandwidth);
    assert_eq!((pre.stream_type, pre.bwt_ext), (0x00, false));
    // S1 000 (T2 SISO); S2 field 1 110 (8K, guard 1/128, 19/256 or 19/128), field 2 0.
    assert_eq!((pre.s1, pre.s2), (0b000, 0b1100));
    assert!(!pre.repetition);
    assert_eq!((pre.guard, pre.papr), (0b100, 0b0000));
    assert_eq!((pre.l1_mod, pre.l1_cod, pre.l1_fec_type), (0b0001, 0, 0));
    // One PLP, one RF channel: 318 information bits, which QPSK carries in 750 cells with N_P2 = 2.
    assert_eq!((pre.l1_post_info_size, pre.l1_post_size), (318, 750));
    assert_eq!((pre.pilot_pattern, pre.tx_id_availability), (0b0110, 0));
    assert_eq!(
        (pre.cell_id, pre.network_id, pre.t2_system_id),
        (0x0001, 0x3001, 0x8001)
    );
    assert_eq!((pre.num_t2_frames, pre.num_data_symbols), (2, 58));
    assert_eq!((pre.regen_flag, pre.post_extension), (0, false));
    assert_eq!((pre.num_rf, pre.current_rf_idx), (1, 0));
    assert_eq!((pre.post_scrambled, pre.t2_base_lite, pre.reserved), (false, false, 0));
    // 64-QAM 010, rate 2/3 010, 64 800-bit frames 01, rotated, TS, a type 1 data PLP in normal mode, three TI-blocks.
    assert_eq!((plp.modulation, plp.cod, plp.fec_type), (0b010, 0b010, 0b01));
    assert!(plp.rotation);
    assert_eq!((plp.plp_id, plp.plp_type, plp.payload_type, plp.plp_mode), (0, 1, 3, 1));
    assert_eq!((plp.num_blocks_max, plp.frame_interval), (36, 1));
    assert_eq!((plp.time_il_length, plp.time_il_type), (3, false));
}

#[test]
fn the_l1_fields_of_the_sender_are_the_configured_ones() {
    let c = TxConfig {
        bandwidth: Bandwidth::B5,
        fft: Fft::K4,
        guard: Guard::G1_16,
        pilots: PilotPattern::Pp5,
        extended: false,
        coding: Coding {
            frame: FecFrame::Short,
            rate: CodeRate::R4_5,
            modulation: Modulation::Qam256,
            rotated: false,
        },
        ti_blocks: 0,
        frame_ms: 40,
        l1_mod: Modulation::Qam16,
        cell_id: 0xBEEF,
        network_id: 0x1234,
        system_id: 0x4321,
    };
    let plan = c.plan().unwrap();
    let (pre, plp) = l1_of(&frames_of(&c, 3), c.bandwidth);
    // 4K with a guard of the family 1/32, 1/16, 1/8, 1/4: S2 field 1 is 010. Guard 1/16 is 001, PP5 is 0100.
    assert_eq!((pre.s1, pre.s2), (0b000, 0b0100));
    assert_eq!((pre.guard, pre.pilot_pattern), (0b001, 0b0100));
    assert_eq!(pre.l1_mod, 0b0010);
    assert_eq!(
        (pre.cell_id, pre.network_id, pre.t2_system_id),
        (0xBEEF, 0x1234, 0x4321)
    );
    assert_eq!(pre.num_data_symbols as usize, plan.l_data);
    // 256-QAM 011, rate 4/5 100, 16 200-bit frames 00, not rotated, no time interleaving.
    assert_eq!((plp.modulation, plp.cod, plp.fec_type), (0b011, 0b100, 0b00));
    assert!(!plp.rotation);
    assert_eq!(plp.num_blocks_max, plan.blocks);
    assert_eq!((plp.time_il_length, plp.time_il_type), (0, false));
    assert_eq!(plan.time_il, TimeIl::Off);
}

#[test]
fn the_useful_bit_rate_matches_the_calculation_by_hand() {
    // The defaults: 8 MHz (T = 7/64 µs), 8K, guard 1/128 (a symbol of 8192 + 64 T), PP7, 64-QAM 2/3 with 64 800-bit
    // frames, QPSK L1-post, frames of 100 ms at most. 100 ms is 914 285 T; without P1 (2048 T) 110 symbols fit, 2 of
    // them P2 symbols. A P2 symbol has 4472 cells (table 47), a data symbol 6698 (table 48), no frame closing symbol.
    // L1 takes 1840 + 750 cells. D_PLP = 2 × 4472 − 2590 + 108 × 6698 = 729 738 cells, which hold 67 FEC blocks of
    // 10 800 cells (723 600). 107 data symbols give 723 040 cells, too few, so the frame keeps 108. K_bch = 43 040, of
    // which 80 bits are the BBHEADER.
    let c = TxConfig::default();
    let plan = c.plan().unwrap();
    assert_eq!((plan.l_data, plan.blocks), (108, 67));
    assert_eq!(plan.time_il, TimeIl::Blocks(3));
    let frame_s: f64 = (2048.0 + 110.0 * 8256.0) * 7.0 / 64e6;
    let by_hand: f64 = 67.0 * (43_040.0 - 80.0) / frame_s;
    assert!((frame_s - 99.554e-3).abs() < 1e-6 && (by_hand - 28.9122e6).abs() < 100.0);
    assert!((plan.bit_rate - by_hand).abs() < 1e-3, "{}", plan.bit_rate);
    let (st, _host) = station(&c, c.bandwidth.rate());
    let (f, problem) = tx_fields(&st);
    assert!(!f.transmitting && !problem);
    assert!((f.tx_bitrate_bps - by_hand).abs() < 1e-3);
    assert_eq!(f.tx_fec_blocks, Some(67));
    assert!((f.tx_frame_s.unwrap().0 - frame_s).abs() < 1e-9);

    // 1.7 MHz (T = 71/131 µs), 2K, guard 1/8 (a symbol of 2048 + 256 T), PP2, QPSK 1/2 with 16 200-bit frames. 100 ms
    // is 184 507 T; without P1 79 symbols fit, 8 of them P2 symbols of 1118 cells. Of the 71 data symbols the last is a
    // frame closing symbol of 1309 cells (table 50), the others have 1532. L1 takes 1840 + 752 cells.
    // D_PLP = 8 × 1118 − 2592 + 70 × 1532 + 1309 = 114 901 cells, which hold 14 FEC blocks of 8100 cells; 70 data
    // symbols hold 113 369 cells, fewer than 113 400. K_bch = 7032.
    let c = narrow();
    let plan = c.plan().unwrap();
    assert_eq!((plan.l_data, plan.blocks), (71, 14));
    let frame_s: f64 = (2048.0 + 79.0 * 2304.0) * 71.0 / 131e6;
    let by_hand: f64 = 14.0 * (7032.0 - 80.0) / frame_s;
    assert!((frame_s - 99.760e-3).abs() < 1e-6 && (by_hand - 975.62e3).abs() < 10.0);
    assert!((plan.bit_rate - by_hand).abs() < 1e-3, "{}", plan.bit_rate);
}

#[test]
fn the_frame_of_the_plan_is_the_frame_of_the_transmitter() {
    let mut cases = vec![TxConfig::default(), narrow()];
    cases.push(TxConfig {
        fft: Fft::K32,
        extended: true,
        frame_ms: 250,
        ti_blocks: 1,
        ..TxConfig::default()
    });
    cases.push(TxConfig {
        fft: Fft::K1,
        guard: Guard::G1_4,
        pilots: PilotPattern::Pp1,
        frame_ms: 10,
        ..narrow()
    });
    for c in cases {
        let plan = c.plan().unwrap();
        let t = c.transmitter(&plan).expect("the plan fits");
        assert_eq!(t.frame_len(), plan.frame_samples, "{c:?}");
        assert!(plan.frame_s <= 0.25 && plan.blocks <= 1023);
        // The transmitter finds the same fewest data symbols for the FEC blocks of the plan.
        let free = c
            .transmitter(&FramePlan { l_data: 0, ..plan })
            .expect("a frame");
        assert_eq!(free.shape().l_data, plan.l_data, "{c:?}");
        // One FEC block more does not fit the longest frame, the time interleaver or PLP_NUM_BLOCKS.
        let nc = c.coding.cells();
        let n_ti = c.ti_blocks.max(1) as usize;
        let more = FramePlan {
            blocks: plan.blocks + 1,
            l_data: 0,
            ..plan
        };
        let longer = c
            .transmitter(&more)
            .map_or(f64::INFINITY, |t| t.frame_len() as f64 / t.rate());
        let limit = (f64::from(c.frame_ms) / 1e3).max(plan.frame_s);
        assert!(
            longer > limit
                || (usize::from(plan.blocks) + 1).div_ceil(n_ti) * nc
                    > sdrkit_stack_dvbt2::tx::TI_MEMORY_CELLS
                || plan.blocks == 1023,
            "{c:?}: {plan:?}"
        );
    }
}

#[test]
fn the_parts_of_a_frame_are_the_frame() {
    let c = narrow();
    let plan = c.plan().unwrap();
    let mut whole = c.transmitter(&plan).unwrap();
    let mut parts = c.transmitter(&plan).unwrap();
    parts.code_ahead(true);
    let (mut a, mut b) = (
        NumberedSource::new(crate::common::PATTERN),
        NumberedSource::new(crate::common::PATTERN),
    );
    for frame in 0..3 {
        let mut x = Vec::new();
        loop {
            whole.part(Some(&mut a), &mut x).unwrap();
            if whole.at_frame_start() {
                break;
            }
        }
        let mut y = Vec::new();
        let mut calls = 0;
        loop {
            parts.part(Some(&mut b), &mut y).unwrap();
            calls += 1;
            if parts.at_frame_start() {
                break;
            }
        }
        assert_eq!(calls, 1 + 8 + plan.l_data);
        assert!(x == y, "frame {frame}");
        assert_eq!(x.len(), plan.frame_samples);
    }
}

/// The power of `x` in each of `bins` equal bands over the sample rate, the lowest frequency first.
fn spectrum(x: &[Complex32], bins: usize) -> Vec<f64> {
    let mut fft = sdrkit_algo_dsp::fft::Fft::forward(bins);
    let mut p = vec![0.0; bins];
    let mut buf = vec![Complex32::default(); bins];
    for seg in x.chunks_exact(bins) {
        for (i, (b, s)) in buf.iter_mut().zip(seg).enumerate() {
            let w = 0.5 - 0.5 * (std::f32::consts::TAU * i as f32 / bins as f32).cos();
            *b = *s * w;
        }
        fft.process(&mut buf);
        for (k, v) in buf.iter().enumerate() {
            p[(k + bins / 2) % bins] += f64::from(v.norm_sqr());
        }
    }
    p
}

#[test]
fn level_peaks_and_occupied_width() {
    let c = TxConfig::default();
    let rate = c.bandwidth.rate();
    let (x, ends, f) = send(&c, rate, &[], 0.5, 0.0);
    assert_eq!((ends, f.tx_packets), (1, 0));
    assert!(f.tx_nulls > 0);
    // The rise is 5 ms; take whole frames of the steady part.
    let frame = c.plan().unwrap().frame_samples;
    let steady = &x[frame..5 * frame];
    let n = steady.len() as f64;
    let power = steady.iter().map(|z| f64::from(z.norm_sqr())).sum::<f64>() / n;
    let mut mags: Vec<f32> = steady.iter().map(|z| z.norm()).collect();
    mags.sort_by(f32::total_cmp);
    let peak = mags[mags.len() - 1];
    let p4 = mags[((1.0 - 1e-4) * n) as usize];
    let above = mags.iter().filter(|&&m| m > KNEE).count() as f64 / n;
    let papr = 20.0 * (f64::from(peak) / power.sqrt()).log10();
    let papr4 = 20.0 * (f64::from(p4) / power.sqrt()).log10();
    eprintln!(
        "rms {:.4}, peak {peak:.3}, PAPR {papr:.1} dB, at 1e-4 {papr4:.1} dB, above the knee {:.5} %",
        power.sqrt(),
        above * 100.0
    );
    assert!((power.sqrt() - f64::from(RMS)).abs() < 0.005);
    assert!(peak < 1.0 && papr < 13.0 && papr4 > 9.0 && papr4 < 12.5 && above < 1e-5);
    // The error of the limiter: each limited sample against the sample before the limiter, which the inverse of the
    // limiter gives.
    let e = steady
        .iter()
        .map(|z| {
            let m = f64::from(z.norm());
            if m <= f64::from(KNEE) {
                return 0.0;
            }
            let before = 0.9 + 0.1 * ((m - 0.9) / 0.1).atanh();
            (before - m).powi(2)
        })
        .sum::<f64>()
        / n;
    let mer_db = 10.0 * (power / e.max(1e-30)).log10();
    eprintln!("limiter MER {mer_db:.1} dB");
    assert!(mer_db > 35.0);
    // The occupied width: 6817 carriers of 64/7 MHz / 8192, 7.61 MHz. The band that holds 99 % of the power, and the
    // power beyond 4.2 MHz from the centre.
    let bins = 4096;
    let p = spectrum(steady, bins);
    let total: f64 = p.iter().sum();
    let hz = rate / bins as f64;
    let mut half = 0;
    let mut inside = p[bins / 2];
    while inside < 0.99 * total {
        half += 1;
        inside += p[bins / 2 - half] + p[bins / 2 + half];
    }
    let width = (2 * half + 1) as f64 * hz;
    let far = (4.2e6 / hz) as usize;
    let outside: f64 = p[..bins / 2 - far].iter().chain(&p[bins / 2 + far..]).sum();
    let outside_db = 10.0 * (outside / total).log10();
    eprintln!(
        "occupied {:.3} MHz by the carriers, {:.3} MHz hold 99 % of the power, {outside_db:.1} dB beyond 4.2 MHz",
        c.occupied_hz() / 1e6,
        width / 1e6
    );
    assert!((c.occupied_hz() - 7.6071e6).abs() < 100.0);
    assert!(width > 7.4e6 && width < 7.62e6, "{width}");
    assert!(outside_db < -30.0, "{outside_db}");
}

#[test]
fn stop_ends_at_a_frame_boundary_with_the_fall() {
    let c = narrow();
    // The rate as a radio states it: whole hertz, 0.42 Hz off 131/71 MHz. The samples go out as they are made.
    let rate = c.bandwidth.rate().round();
    assert!(rate != c.bandwidth.rate());
    let plan = c.plan().unwrap();
    let (mut st, host) = station(&c, rate);
    call(&mut st, "start", None).unwrap();
    run(&mut st, rate, 0.25, |_| {});
    call(&mut st, "stop", None).unwrap();
    // The transmission is ending: a start is refused until it has ended.
    let e = call(&mut st, "start", None).unwrap_err();
    assert_eq!(refusal(&e), Some(TxError::Ending));
    run(&mut st, rate, 0.25, |_| {});
    assert!(!tx_fields(&st).0.transmitting);
    let (x, ends) = host.sent();
    assert_eq!(ends, 1);
    // Whole frames, then the fall of 5 ms over the start of the next frame.
    let fall = (RISE_S * rate).round() as usize;
    assert_eq!((x.len() - fall) % plan.frame_samples, 0, "{} samples", x.len());
    let frames = (x.len() - fall) / plan.frame_samples;
    // 0.25 s is in the third frame of 99.76 ms.
    assert_eq!(frames, 3);
    let rms = |s: &[Complex32]| {
        (s.iter().map(|z| f64::from(z.norm_sqr())).sum::<f64>() / s.len() as f64).sqrt()
    };
    let end = x.len();
    assert!(rms(&x[end - fall - 2000..end - fall]) > 0.2);
    assert!(rms(&x[end - 50..]) < 0.01);
    assert!(x[end - 1].norm() < 1e-3);
    // The stack sends again after the end.
    call(&mut st, "start", None).expect("starts again");
}

#[test]
fn refusals_of_start_and_of_append() {
    let c = TxConfig::default();
    let (mut st, _host) = station(&c, c.bandwidth.rate());
    let full: Vec<u8> = host_stream(1e6, QUEUE_PACKETS)
        .into_iter()
        .flatten()
        .collect();
    call(&mut st, "append", Some(&full)).expect("the queue takes 6000 packets");
    assert_eq!(tx_fields(&st).0.tx_queue as usize, QUEUE_PACKETS);
    let one = &full[..TS_SIZE];
    let e = call(&mut st, "append", Some(one)).unwrap_err();
    assert_eq!(
        refusal(&e),
        Some(TxError::Queue(QueueError::Full {
            packets: 1,
            room: 0
        }))
    );
    let e = call(&mut st, "append", Some(&full[..100])).unwrap_err();
    assert_eq!(
        refusal(&e),
        Some(TxError::Queue(QueueError::NotWhole { bytes: 100 }))
    );
    let mut bad = full[..2 * TS_SIZE].to_vec();
    bad[TS_SIZE] = 0x48;
    let e = call(&mut st, "append", Some(&bad)).unwrap_err();
    assert_eq!(
        refusal(&e),
        Some(TxError::Queue(QueueError::NoSync { packet: 1 }))
    );
    // The switch.
    st.set_transmit(false);
    assert!(matches!(
        call(&mut st, "start", None),
        Err(CommandError::TransmitOff)
    ));
    assert!(!tx_fields(&st).0.transmitting);

    // A session rate below the occupied width over 0.9.
    let (mut st, _host) = station(&c, 8e6);
    let e = call(&mut st, "start", None).unwrap_err();
    assert!(matches!(
        refusal(&e),
        Some(TxError::RateTooLow { rate, needs }) if rate == 8e6 && (needs - 7.6071e6 / 0.9).abs() < 200.0
    ));

    // A pilot pattern that table 58 does not allow with 8K and guard 1/128; a guard that 2K does not take; extended
    // carriers with 2K.
    for (bad, rule) in [
        (
            TxConfig {
                pilots: PilotPattern::Pp1,
                ..c
            },
            Rule::PilotsForGuard,
        ),
        (
            TxConfig {
                fft: Fft::K2,
                ..c
            },
            Rule::GuardForFft,
        ),
        (
            TxConfig {
                extended: true,
                ..narrow()
            },
            Rule::ExtendedCarriers,
        ),
    ] {
        let (mut st, host) = station(&bad, 10e6);
        let (f, problem) = tx_fields(&st);
        assert!(problem && f.tx_bitrate_bps == 0.0 && f.tx_frame_s.is_none());
        let e = call(&mut st, "start", None).unwrap_err();
        assert_eq!(refusal(&e), Some(TxError::Settings(T2Error::Rule(rule))));
        run(&mut st, 10e6, 0.05, |_| {});
        assert!(host.sent().0.is_empty());
    }
}

#[test]
fn on_a_live_radio_the_first_sample_states_its_time_on_air() {
    let c = narrow();
    let rate = 2e6;
    let (mut st, host) = station_with(&c, rate, Some(Duration::from_millis(50)));
    call(&mut st, "start", None).unwrap();
    run(&mut st, rate, 0.1, |_| {});
    let w = host.writes.lock().unwrap();
    let first = w.iter().position(|(s, _, _)| !s.is_empty()).expect("samples");
    let start = w[first].2.expect("a stated start");
    assert!(start > 1_800_000_000.0 && start < 1_800_000_001.0, "{start}");
    assert_eq!(w.iter().filter(|(_, _, s)| s.is_some()).count(), 1);
}

#[test]
fn the_receiver_without_the_sender_has_no_transmit_commands() {
    let cfg = config(&[]);
    let rx = sdrkit_stack_dvbt2::make_receiver(&cfg).expect("the receiver alone");
    assert!(rx.commands().id("start").is_err());
    assert!(rx.commands().id("service.select").is_ok());
    let st = sdrkit_stack_dvbt2::receiver(&cfg).expect("the stack");
    for key in ["start", "stop", "append", "service.select", "track.select"] {
        assert!(st.commands().id(key).is_ok(), "{key}");
    }
    assert!(st.transmits());
}
