//! The sender of `dvbt`: the stack's own receiver returns the host's transport stream bit for bit (null packets and PCR
//! fields aside), the TPS word matches the worked example of the specification, the useful bit rate is stated, the level,
//! the peaks and the modulation error ratio of the signal, a signal of null packets without input, and the refusals.
//! Nothing goes to a radio.

use crate::common::{choice, config, drain_packets, receiver, Case};
use sdrkit_components::tx::SOFT_KNEE as KNEE;
use sdrkit_core::owner::{self, Owner};
use sdrkit_core::interface::ReadFields;
use sdrkit_core::{Clock, CommandError, Complex32, InputInfo, Output, Receiver, Value};
use sdrkit_graph::components::{HostData, HostPort, HostStreams};
use sdrkit_graph::{RunContext, StackGraph};
use sdrkit_proto_mpegts::packet::{parse, NULL_PID, TS_SIZE};
use sdrkit_proto_mpegts::sendqueue::{QueueError, QUEUE_PACKETS};
use sdrkit_stack_dvbt::generator::test_mux;
use sdrkit_stack_dvbt::params::*;
use sdrkit_stack_dvbt::settings::{BandwidthChoice, StreamChoice};
use sdrkit_stack_dvbt::transmit::{TxConfig, TxError, RMS};
use sdrkit_stack_dvbt::typed::{DvbtFields, DvbtTxFields};
use sdrkit_stack_dvbt::{receiver, tps};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// The dial of the transmitting stack.
const DIAL: f64 = 474e6;

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

/// The values of the `tx_*` settings of `c`.
fn tx_settings(c: &TxConfig) -> Vec<(&'static str, Value)> {
    vec![
        ("tx_bandwidth", choice(c.bandwidth)),
        ("tx_mode", choice(c.mode)),
        ("tx_guard", choice(c.guard)),
        ("tx_constellation", choice(c.constellation)),
        ("tx_hierarchy", choice(c.hierarchy)),
        ("tx_rate_hp", choice(c.rate_hp)),
        ("tx_rate_lp", choice(c.rate_lp)),
        ("tx_cell_id", Value::I64(i64::from(c.cell_id))),
    ]
}

/// The `dvbt` stack with the sender settings of `c` at `rate`, the transmitter on, and its host.
fn station(c: &TxConfig, rate: f64) -> (StackGraph, Arc<Host>) {
    station_with(c, rate, None)
}

/// [`station`]; with `lead`, on a live radio whose host times bursts with that lead.
fn station_with(c: &TxConfig, rate: f64, lead: Option<Duration>) -> (StackGraph, Arc<Host>) {
    let mut st = receiver::receiver(&config(&tx_settings(c))).expect("the stack takes the settings");
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

fn tx_fields(st: &StackGraph) -> DvbtTxFields {
    DvbtTxFields::read(Owner::results(st).fields()).expect("the sender's fields")
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
    let mut src = test_mux(7, "Loop service", bit_rate);
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
    if parse(p).and_then(|x| x.adaptation).and_then(|a| a.pcr).is_some() {
        q[6..12].fill(0);
    }
    q
}

/// The signal to noise ratio of the receiving radio, dB.
const SNR_DB: f64 = 40.0;

/// The non-null packets that the receiver of `stream` decodes from `x` at `rate` with noise at [`SNR_DB`], a channel of
/// `bw`, the count of all packets, and its results.
fn receive(
    x: &[Complex32],
    rate: f64,
    bw: Bandwidth,
    tps: TpsParams,
    stream: StreamChoice,
) -> (Vec<[u8; TS_SIZE]>, usize, sdrkit_core::Results) {
    let mut c = Case::new(bw, tps);
    c.lp = stream == StreamChoice::Lp;
    c.rx_bandwidth = Some(BandwidthChoice::of(bw));
    let (mut rx, mut tap) = receiver(&c, rate);
    let mut buf = sdrkit_graph::Buffer::new(&sdrkit_proto_mpegts::stream::kind());
    let mut packets = Vec::new();
    let mut out = Output::default();
    // Noise 40 dB below the signal over the occupied band, as a radio always adds.
    let mut x = x.to_vec();
    let p = x.iter().map(|z| f64::from(z.norm_sqr())).sum::<f64>() / x.len() as f64;
    let occupied = tps.mode.carriers() as f64 * bw.natural_rate() / tps.mode.fft() as f64;
    sdrkit_dev_test::Awgn::new(p, SNR_DB, occupied, rate, 3).process(&mut x);
    for piece in x.chunks(16384) {
        rx.feed(piece, &mut out);
        out.clear();
        drain_packets(&mut tap, &mut buf, &mut packets);
    }
    rx.finish(&mut out);
    drain_packets(&mut tap, &mut buf, &mut packets);
    let all = packets.len();
    // The fall at the end of a transmission cuts the last blocks: their packets carry the transport_error_indicator, and
    // they come after every packet of the host. The fall and the outer interleaver span at most 8 ms of packets.
    let damaged = |p: &[u8; TS_SIZE]| p[1] & 0x80 != 0;
    let first = packets.iter().position(damaged).unwrap_or(packets.len());
    let last_data = packets.iter().rposition(|p| pid(p) != NULL_PID && !damaged(p));
    let bad: Vec<usize> = (0..packets.len()).filter(|&i| damaged(&packets[i])).collect();
    let first_data = packets.iter().position(|p| pid(p) != NULL_PID && !damaged(p));
    assert!(
        last_data.is_none_or(|l| l < first),
        "a damaged packet before the end: data {first_data:?} to {last_data:?} of {}, damaged {:?}",
        packets.len(),
        &bad[..bad.len().min(40)]
    );
    let most = tps.net_bitrate(bw, c.lp) * 0.008 / 1504.0 + 20.0;
    assert!(((packets.len() - first) as f64) < most, "{} damaged packets", packets.len() - first);
    packets.retain(|p| pid(p) != NULL_PID && p[1] & 0x80 == 0);
    (packets, all, Owner::results(&rx).clone())
}

/// One loop: the sender of `c` at `rate` with the streams `hp` (and `lp`), 0.4 s of null packets first, the packets fed as
/// the queue has room, then null packets until the interleavers are flushed; the samples sent and the end marks.
fn send(
    c: &TxConfig,
    rate: f64,
    hp: &[[u8; TS_SIZE]],
    lp: &[[u8; TS_SIZE]],
    lead_s: f64,
    tail_s: f64,
) -> (Vec<Complex32>, usize, DvbtTxFields) {
    let (mut st, host) = station(c, rate);
    call(&mut st, "start", None).expect("starts");
    run(&mut st, rate, lead_s, |_| {});
    let (mut at_hp, mut at_lp) = (0, 0);
    let feed = |st: &mut StackGraph, key: &str, from: &[[u8; TS_SIZE]], at: &mut usize| {
        let f = tx_fields(st);
        let waiting = if key == "append" { f.tx_queue } else { f.tx_queue_lp } as usize;
        let k = (QUEUE_PACKETS - waiting).min(from.len() - *at).min(2000);
        if k > 0 {
            let bytes: Vec<u8> = from[*at..*at + k].iter().flatten().copied().collect();
            call(st, key, Some(&bytes)).expect("room in the queue");
            *at += k;
        }
    };
    while at_hp < hp.len() || at_lp < lp.len() {
        run(&mut st, rate, 0.05, |st| {
            feed(st, "append", hp, &mut at_hp);
            if !lp.is_empty() {
                feed(st, "append_lp", lp, &mut at_lp);
            }
        });
    }
    let mut waited = 0.0;
    while tx_fields(&st).tx_queue + tx_fields(&st).tx_queue_lp > 0 && waited < 5.0 {
        run(&mut st, rate, 0.05, |_| {});
        waited += 0.05;
    }
    run(&mut st, rate, tail_s, |_| {});
    let fields = tx_fields(&st);
    call(&mut st, "stop", None).unwrap();
    run(&mut st, rate, 0.3, |_| {});
    assert!(!tx_fields(&st).transmitting, "stop ends the transmission");
    let (x, ends) = host.sent();
    (x, ends, fields)
}

/// The loop of `c` at `rate`: what the receiver returns equals what the host gave.
fn loop_test(c: &TxConfig, rate: f64, packets: usize) -> (Vec<Complex32>, sdrkit_core::Results) {
    let hp = host_stream(c.bit_rate(false), packets);
    let (x, ends, f) = send(c, rate, &hp, &[], 0.4, 0.15);
    assert_eq!(ends, 1, "one end mark");
    assert_eq!(f.tx_packets as usize, hp.len());
    assert!(f.tx_nulls > 0 && f.tx_pcr_restamped > 0, "{f:?}");
    let (got, _, r) = receive(&x, rate, c.bandwidth, c.tps(), StreamChoice::Hp);
    let want: Vec<_> = hp.iter().map(masked).collect();
    let got: Vec<_> = got.iter().map(masked).collect();
    assert_eq!(got.len(), want.len(), "packets returned");
    assert!(got == want, "the packets differ");
    (x, r)
}

fn mer(r: &sdrkit_core::Results) -> f64 {
    DvbtFields::read(r.fields())
        .and_then(|f| f.mer_db)
        .expect("a modulation error ratio")
}

#[test]
fn loop_2k_qpsk_half_at_7_mhz_resampled() {
    let c = TxConfig {
        bandwidth: Bandwidth::B7,
        mode: Mode::K2,
        guard: Guard::G4,
        constellation: Constellation::Qpsk,
        hierarchy: Hierarchy::None,
        rate_hp: CodeRate::R12,
        rate_lp: CodeRate::R12,
        cell_id: 0x1001,
    };
    let rate = Bandwidth::B8.natural_rate();
    let (_, r) = loop_test(&c, rate, 2500);
    eprintln!("2K QPSK 1/2 at 7 MHz, sent at {rate} sps: MER {:.1} dB", mer(&r));
    assert!(mer(&r) > 35.0);
    let d = DvbtFields::read(r.fields()).unwrap();
    assert_eq!((d.cell_id, d.tps_length), (Some(0x1001), Some(31)));
}

#[test]
fn loop_8k_64qam_two_thirds_at_8_mhz() {
    let c = TxConfig {
        bandwidth: Bandwidth::B8,
        mode: Mode::K8,
        guard: Guard::G8,
        constellation: Constellation::Qam64,
        hierarchy: Hierarchy::None,
        rate_hp: CodeRate::R23,
        rate_lp: CodeRate::R23,
        cell_id: 0,
    };
    let rate = Bandwidth::B8.natural_rate();
    let (_, r) = loop_test(&c, rate, 5000);
    eprintln!("8K 64-QAM 2/3 at 8 MHz: MER {:.1} dB", mer(&r));
    assert!(mer(&r) > 35.0);
}

#[test]
fn loop_hierarchical_alpha_2_both_streams() {
    let c = TxConfig {
        bandwidth: Bandwidth::B8,
        mode: Mode::K2,
        guard: Guard::G8,
        constellation: Constellation::Qam64,
        hierarchy: Hierarchy::A2,
        rate_hp: CodeRate::R12,
        rate_lp: CodeRate::R23,
        cell_id: 0,
    };
    let rate = Bandwidth::B8.natural_rate();
    let hp = host_stream(c.bit_rate(false), 1500);
    let lp = host_stream(c.bit_rate(true), 2500);
    let (x, ends, f) = send(&c, rate, &hp, &lp, 0.4, 0.15);
    assert_eq!((ends, f.tx_packets as usize), (1, hp.len() + lp.len()));
    for (stream, want) in [(StreamChoice::Hp, &hp), (StreamChoice::Lp, &lp)] {
        let (got, _, _) = receive(&x, rate, c.bandwidth, c.tps(), stream);
        let want: Vec<_> = want.iter().map(masked).collect();
        let got: Vec<_> = got.iter().map(masked).collect();
        assert!(got == want, "{stream:?}: {} packets of {}", got.len(), want.len());
    }
}

#[test]
fn without_input_the_signal_carries_null_packets() {
    let c = TxConfig {
        mode: Mode::K2,
        constellation: Constellation::Qpsk,
        rate_hp: CodeRate::R12,
        ..TxConfig::default()
    };
    let rate = c.bandwidth.natural_rate();
    let (x, ends, f) = send(&c, rate, &[], &[], 0.6, 0.0);
    assert_eq!((ends, f.tx_packets), (1, 0));
    let (got, all, r) = receive(&x, rate, c.bandwidth, c.tps(), StreamChoice::Hp);
    assert!(got.is_empty() && all > 500, "{} packets, {all} in all", got.len());
    let d = DvbtFields::read(r.fields()).unwrap();
    assert_eq!(
        (d.fft, d.constellation, d.code_rate_hp),
        (Some(Mode::K2), Some(Constellation::Qpsk), Some(CodeRate::R12))
    );
}

#[test]
fn a_full_queue_refuses_and_appends_need_whole_packets() {
    let (mut st, _host) = station(&TxConfig::default(), Bandwidth::B8.natural_rate());
    let full: Vec<u8> = host_stream(1e6, QUEUE_PACKETS).into_iter().flatten().collect();
    call(&mut st, "append", Some(&full)).expect("the queue takes 6000 packets");
    assert_eq!(tx_fields(&st).tx_queue as usize, QUEUE_PACKETS);
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
    let e = call(&mut st, "append_lp", Some(one)).unwrap_err();
    assert_eq!(refusal(&e), Some(TxError::NotHierarchical));
    st.set_transmit(false);
    assert!(matches!(
        call(&mut st, "start", None),
        Err(CommandError::TransmitOff)
    ));
}

#[test]
fn on_a_live_radio_the_first_sample_states_its_time_on_air() {
    let c = TxConfig::default();
    let rate = c.bandwidth.natural_rate();
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
fn the_useful_bit_rate_is_stated_before_sending() {
    // Worked example 2 of the specification.
    let c = TxConfig::default();
    let (st, _host) = station(&c, Bandwidth::B8.natural_rate());
    let f = tx_fields(&st);
    assert!((f.tx_bitrate_bps - 22.12e6).abs() < 0.01e6, "{}", f.tx_bitrate_bps);
    assert!(!f.transmitting && f.tx_bitrate_lp_bps.is_none());
    let c = TxConfig {
        bandwidth: Bandwidth::B7,
        mode: Mode::K2,
        constellation: Constellation::Qam16,
        ..TxConfig::default()
    };
    assert!((c.bit_rate(false) - 12.90196e6).abs() < 10.0);
}

/// The bits s1 to s67 of a frame of the worked example, as the specification writes them.
fn example_frame(sync: &str, frame: &str, cell: &str, bch: &str) -> Vec<u8> {
    [sync, "011111", frame, "100000010011001", cell, "000000", bch]
        .concat()
        .bytes()
        .map(|b| b - b'0')
        .collect()
}

#[test]
fn the_tps_word_matches_the_worked_example() {
    let c = TxConfig {
        cell_id: 0x1001,
        ..TxConfig::default()
    };
    let p = c.tps();
    let one = example_frame("0011010111101110", "00", "00010000", "11011000001001");
    let two = example_frame("1100101000010001", "01", "00000001", "11010111011110");
    assert_eq!(tps::encode(&p, 0)[1..].to_vec(), one, "frame 1");
    assert_eq!(tps::encode(&p, 1)[1..].to_vec(), two, "frame 2");
}

#[test]
fn level_peaks_and_the_error_of_the_limiter() {
    let c = TxConfig::default();
    let rate = c.bandwidth.natural_rate();
    let (x, _, _) = send(&c, rate, &[], &[], 0.5, 0.0);
    // The rise is 5 ms; take the steady part.
    let steady = &x[(0.01 * rate) as usize..(0.45 * rate) as usize];
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
    assert!(peak < 1.0 && papr4 > 9.0 && papr4 < 12.5 && above < 1e-5);
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
}
