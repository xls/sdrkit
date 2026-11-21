//! The DVB-S sender of `dvbs`: from its lock on, the stack's own receiver returns the transport stream bit for bit (PCR
//! fields aside), null packets included, at every code rate and several symbol rates, within a stated lock time; the useful bit rate of EN 300 421, the spectrum, the
//! level, the end of a transmission, and the refusals. Nothing goes to a radio.

use crate::common::*;
use sdrkit_algo_dsp::spectrum::{welch, Scaling};
use sdrkit_algo_dsp::window::Window;
use sdrkit_core::interface::ReadFields;
use sdrkit_core::owner::{self, Owner};
use sdrkit_core::verify::CaseSetting;
use sdrkit_core::{Clock, CommandError, Complex32, InputInfo, Output, Receiver, Value};
use sdrkit_graph::components::{HostData, HostPort, HostStreams};
use sdrkit_graph::{RunContext, StackGraph};
use sdrkit_proto_dvbs2::inner::PunctureRate;
use sdrkit_proto_dvbs2::s1encode::useful_bit_rate;
use sdrkit_proto_dvbs2::typed::Dvbs2Fields as D;
use sdrkit_proto_dvbs2::settings::{SYMBOL_RATE, TX_INVERSION, TX_RATE, TX_SYMBOL_RATE};
use sdrkit_proto_mpegts::packet::{parse, NULL_PID, TS_SIZE};
use sdrkit_proto_mpegts::sendqueue::{QueueError, QUEUE_PACKETS};
use sdrkit_proto_mpegts::source::{Data, Fill, PacketSource, TestMux, TestService, TestSound};
use sdrkit_stack_dvbcs::dvbs::{self, DVBS};
use sdrkit_stack_dvbcs::dvbs_transmit::{TxConfig, TxError, RISE_S, RMS};
use sdrkit_stack_dvbcs::typed::DvbsTxFields;
use std::sync::{Arc, Mutex};

/// The dial of the transmitting stack: the 23 cm amateur band.
pub(super) const DIAL: f64 = 1255e6;

/// One write to the transmit stream: its samples and its end mark.
pub(super) type Write = (Vec<Complex32>, bool);

/// The host: what the stack writes to the transmit stream, with the end marks.
#[derive(Default)]
pub(super) struct Host {
    writes: Mutex<Vec<Write>>,
}

impl HostStreams for Host {
    fn write(&self, port: HostPort, data: HostData<'_>) {
        if let (HostPort::Transmit, HostData::Iq { samples, end, .. }) = (port, data) {
            self.writes.lock().unwrap().push((samples.to_vec(), end));
        }
    }
}

impl Host {
    /// Every sample written, and the number of end marks.
    pub(super) fn sent(&self) -> (Vec<Complex32>, usize) {
        let w = self.writes.lock().unwrap();
        let x = w.iter().flat_map(|(s, _)| s.iter().copied()).collect();
        (x, w.iter().filter(|(_, e)| *e).count())
    }
}

/// The `dvbs` stack with the sender settings of `c` at `rate`, the transmitter on, and its host.
fn station(c: &TxConfig, rate: f64) -> (StackGraph, Arc<Host>) {
    let cfg = config(
        &DVBS,
        &[
            CaseSetting::new(&TX_SYMBOL_RATE, c.symbol_rate),
            CaseSetting::new(&TX_RATE, c.rate),
            CaseSetting::new(&TX_INVERSION, c.inversion),
        ],
    );
    let mut st = dvbs::receiver(&cfg).expect("the stack takes the settings");
    st.configure(&InputInfo::new(rate, DIAL, DIAL).recording()).unwrap();
    let host = Arc::new(Host::default());
    let mut ctx: RunContext = st.graph().context().clone();
    ctx.streams = Some(host.clone());
    ctx.clock = Clock::fixed(1_800_000_000.0);
    st.set_context(ctx);
    assert!(st.transmits());
    st.set_transmit(true);
    (st, host)
}

/// Runs command `key` of `st` with the bytes `packets` as its argument `packets` when given.
pub(super) fn call(st: &mut StackGraph, key: &str, packets: Option<&[u8]>) -> Result<(), CommandError> {
    let id = st.commands().id(key).unwrap();
    let mut a = st.commands().def(id).unwrap().new_args();
    if let Some(p) = packets {
        let i = a.decl().id("packets").unwrap();
        a.set_value(i, &Value::Bytes(Arc::from(p))).unwrap();
    }
    owner::invoke(st, id, &a).map(|_| ())
}

/// The refusal of the sender inside a command error.
pub(super) fn refusal(e: &CommandError) -> Option<TxError> {
    match e {
        CommandError::Failed { source } => source.downcast_ref::<TxError>().copied(),
        _ => None,
    }
}

pub(super) fn tx_fields(st: &StackGraph) -> DvbsTxFields {
    DvbsTxFields::read(Owner::results(st).fields()).expect("the sender's fields")
}

/// Feeds `seconds` of silence at `rate` in pieces of 0.05 s; before each piece `top_up` may append packets.
pub(super) fn run(st: &mut StackGraph, rate: f64, seconds: f64, mut top_up: impl FnMut(&mut StackGraph)) {
    let piece = vec![Complex32::default(); (rate * 0.05) as usize];
    let mut out = Output::default();
    for _ in 0..(seconds / 0.05).round() as usize {
        top_up(st);
        out.clear();
        st.feed(&piece, &mut out);
    }
}

pub(super) fn pid(p: &[u8]) -> u16 {
    (u16::from(p[1] & 0x1F) << 8) | u16::from(p[2])
}

/// The packets of the host: a test multiplex (tables and numbered data, and above 300 kbit/s a sound with its PCR paced
/// for `bit_rate`), its null packets left out.
pub(super) fn host_stream(bit_rate: f64, count: usize) -> Vec<[u8; TS_SIZE]> {
    let mut b = TestMux::builder(1)
        .service(TestService::tv(7, "Loop service", 1))
        .data(0x0101, Data::Numbered(Fill::I31N7))
        .data_every(3);
    if bit_rate > 300e3 {
        b = b.sound(TestSound {
            pid: 0x0102,
            mux_bit_rate: bit_rate,
        });
    }
    let mut src = b.build().expect("the test multiplex");
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

/// `p` with its PCR field set to zero when it has one.
pub(super) fn masked(p: &[u8; TS_SIZE]) -> [u8; TS_SIZE] {
    let mut q = *p;
    if parse(p).and_then(|x| x.adaptation).and_then(|a| a.pcr).is_some() {
        q[6..12].fill(0);
    }
    q
}

/// One transmission of `c` at `rate`: `lead_s` of null packets, the packets of `hp` fed as the queue has room, then
/// `tail_s` of null packets and `stop`. The samples sent, the end marks, and the fields before `stop`.
fn send(
    c: &TxConfig,
    rate: f64,
    hp: &[[u8; TS_SIZE]],
    lead_s: f64,
    tail_s: f64,
) -> (Vec<Complex32>, usize, DvbsTxFields) {
    send_from(station(c, rate), rate, hp, lead_s, tail_s)
}

/// [`send`] from a stack that is ready to start, with its host.
pub(super) fn send_from(
    (mut st, host): (StackGraph, Arc<Host>),
    rate: f64,
    hp: &[[u8; TS_SIZE]],
    lead_s: f64,
    tail_s: f64,
) -> (Vec<Complex32>, usize, DvbsTxFields) {
    call(&mut st, "start", None).expect("starts");
    run(&mut st, rate, lead_s, |_| {});
    let mut at = 0;
    while at < hp.len() {
        run(&mut st, rate, 0.05, |st| {
            let k = (QUEUE_PACKETS - tx_fields(st).tx_queue as usize)
                .min(hp.len() - at)
                .min(2000);
            if k > 0 {
                let bytes: Vec<u8> = hp[at..at + k].iter().flatten().copied().collect();
                call(st, "append", Some(&bytes)).expect("room in the queue");
                at += k;
            }
        });
    }
    let mut waited = 0.0;
    while tx_fields(&st).tx_queue > 0 && waited < 20.0 {
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

/// The packets that the stack's receiver delivers from `x` at `rate` with noise 30 dB below the signal in its band, and
/// whether it found the spectrum inverted. With `set_rate` the receiver has the symbol rate of `c` set; without, it finds it.
fn receive(x: &[Complex32], rate: f64, c: &TxConfig, set_rate: bool) -> (Vec<[u8; TS_SIZE]>, Option<bool>) {
    let mut x = x.to_vec();
    let p = x.iter().map(|z| f64::from(z.norm_sqr())).sum::<f64>() / x.len() as f64;
    sdrkit_dev_test::Awgn::new(p, 30.0, c.symbol_rate * 1.35, rate, 5).process(&mut x);
    let settings = [CaseSetting::new(&SYMBOL_RATE, c.symbol_rate)];
    let settings = if set_rate { &settings[..] } else { &[] };
    let mut rx = Rx::new(&DVBS, settings, &recording(rate, DIAL));
    rx.feed(&x, 16384);
    rx.finish();
    let inverted = rx.at(&D::INVERTED);
    (rx.packets, inverted)
}

/// The symbols that the receiver may take to lock at `rate` samples per second on a carrier of `symbol_rate`: two surveys
/// of its front end (48 spectra of 4096 samples or 4 ms each; the first survey of a transmission sees the start of the
/// interleaver and may be refused), and [`LOCK_SYMBOLS`] for the symbol clock, the code rate and the packet sync.
fn lock_bound(symbol_rate: f64, rate: f64) -> f64 {
    let survey = 48.0 * (0.004 * rate).max(4096.0);
    2.0 * survey * symbol_rate / rate + LOCK_SYMBOLS
}

/// The symbols after the surveys in which the receiver locks.
const LOCK_SYMBOLS: f64 = 30_000.0;

/// What a loop sends.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Payload {
    /// Null packets only, for the time of the lock bound and this many packets more.
    Nulls(usize),
    /// This many packets of the host after those of the lock bound.
    Mixed(usize),
}

/// The loop of `c` at `rate`: from its lock to the end of the transmission the receiver returns every packet bit for bit
/// (PCR fields aside), the null packets too, and it locks within [`lock_bound`]. The symbols before the lock are counted as
/// all symbols sent less those of the packets returned, so the packets that the end of the transmission cuts count as
/// lock time. Returns that count.
fn loop_test(c: &TxConfig, rate: f64, payload: Payload, set_rate: bool) -> f64 {
    let what = format!(
        "{:?} at {} sym/s, {rate} samples/s, {payload:?}, rate set: {set_rate}",
        c.rate, c.symbol_rate
    );
    let packet_symbols = 204.0 * 8.0 / (2.0 * c.rate.value());
    let bound = lock_bound(c.symbol_rate, rate);
    let warm = (bound / packet_symbols).ceil() as usize;
    let warm_s = bound / c.symbol_rate;
    let (hp, lead_s) = match payload {
        Payload::Nulls(n) => (Vec::new(), warm_s + n as f64 * packet_symbols / c.symbol_rate),
        Payload::Mixed(n) => (host_stream(c.bit_rate(), warm + n), 0.05),
    };
    let (x, ends, f) = send(c, rate, &hp, lead_s, 0.2);
    assert_eq!(ends, 1, "{what}: one end mark");
    assert_eq!(f.tx_packets as usize, hp.len(), "{what}");
    assert!(f.tx_nulls > 0, "{what}: {f:?}");
    if c.bit_rate() > 300e3 && !hp.is_empty() {
        assert!(f.tx_pcr_restamped > 0, "{what}: {f:?}");
    }
    let (mut all, inverted) = receive(&x, rate, c, set_rate);
    assert_eq!(inverted, Some(c.inversion), "{what}: spectral inversion");
    // The end of the transmission cuts the last packets in the interleaver: the receiver marks them as in error.
    let cut = all.iter().rev().take_while(|p| p[1] & 0x80 != 0).count();
    assert!(cut <= 12, "{what}: {cut} packets in error at the end");
    all.truncate(all.len() - cut);
    assert!(all.iter().all(|p| p[1] & 0x80 == 0), "{what}: a packet in error after the lock");
    let sent = x.len() as f64 / rate * c.symbol_rate;
    let lock = sent - all.len() as f64 * packet_symbols;
    eprintln!("{what}: lock in {lock:.0} symbols of {bound:.0}");
    assert!(lock <= bound, "{what}: lock in {lock:.0} symbols, bound {bound:.0}");
    let null = sdrkit_proto_mpegts::source::null_packet();
    let (nulls, got): (Vec<[u8; TS_SIZE]>, Vec<[u8; TS_SIZE]>) =
        all.iter().partition(|p| pid(&p[..]) == NULL_PID);
    assert!(nulls.iter().all(|p| *p == null), "{what}: a null packet differs");
    let got: Vec<_> = got.iter().map(masked).collect();
    let want: Vec<_> = hp.iter().map(masked).collect();
    assert!(got.len() <= want.len(), "{what}: {} packets returned of {}", got.len(), want.len());
    assert!(got[..] == want[want.len() - got.len()..], "{what}: the packets differ");
    if let Payload::Mixed(n) = payload {
        assert!(got.len() >= n, "{what}: {} packets returned of {}", got.len(), want.len());
    }
    lock
}

/// The loop of `c` at `rate` with a mixed payload and with null packets only. The null packets carry no number, so a
/// loss among them shows only in their count: the lock of the null loop is within 8 packets of that of the mixed loop,
/// whose packets are compared one by one.
fn loop_both(c: &TxConfig, rate: f64, packets: usize, set_rate: bool) {
    let mixed = loop_test(c, rate, Payload::Mixed(packets), set_rate);
    let nulls = loop_test(c, rate, Payload::Nulls(packets), set_rate);
    let packet_symbols = 204.0 * 8.0 / (2.0 * c.rate.value());
    assert!(
        (mixed - nulls).abs() <= 8.0 * packet_symbols,
        "{:?} at {} sym/s: lock in {mixed:.0} symbols with a mixed payload, {nulls:.0} with null packets",
        c.rate,
        c.symbol_rate
    );
}

fn cfg(symbol_rate: f64, rate: PunctureRate) -> TxConfig {
    TxConfig {
        symbol_rate,
        rate,
        inversion: false,
    }
}

#[test]
fn loop_every_code_rate_at_125_ksym_s_with_the_rate_set() {
    for r in PunctureRate::ALL {
        loop_both(&cfg(125e3, r), 2e6, 150, true);
    }
}

#[test]
fn loop_every_code_rate_at_125_ksym_s() {
    for r in PunctureRate::ALL {
        loop_both(&cfg(125e3, r), 2e6, 150, false);
    }
}

#[test]
fn loop_every_code_rate_at_2_samples_per_symbol() {
    for r in PunctureRate::ALL {
        loop_both(&cfg(1e6, r), 2e6, 600, false);
    }
}

#[test]
fn loop_every_code_rate_at_2_msym_s() {
    for r in PunctureRate::ALL {
        loop_test(&cfg(2e6, r), 4e6, Payload::Mixed(1000), false);
    }
    loop_both(&cfg(2e6, PunctureRate::R34), 4e6, 1000, false);
}

#[test]
fn loop_two_thirds_at_5_msym_s_inverted() {
    let c = TxConfig {
        inversion: true,
        ..cfg(5e6, PunctureRate::R23)
    };
    loop_test(&c, 10e6, Payload::Mixed(3000), false);
}

#[test]
fn the_useful_bit_rate_follows_the_standard() {
    // Worked example 3 of the specification: QPSK 3/4 at 27.5 Msym/s carries 38.015 Mbit/s.
    let r = useful_bit_rate(27.5e6, PunctureRate::R34);
    assert!((r - 38.015e6).abs() < 1e3, "{r}");
    // Every rate: symbol rate × 2 × rate × 188 / 204.
    for (k, n, rate) in [
        (1.0, 2.0, PunctureRate::R12),
        (2.0, 3.0, PunctureRate::R23),
        (3.0, 4.0, PunctureRate::R34),
        (5.0, 6.0, PunctureRate::R56),
        (7.0, 8.0, PunctureRate::R78),
    ] {
        let want = 1e6 * 2.0 * k / n * 188.0 / 204.0;
        assert!((useful_bit_rate(1e6, rate) - want).abs() < 1e-6);
    }
    // The stack states the rate of its settings before it sends.
    let (st, _host) = station(&cfg(333e3, PunctureRate::R12), 2e6);
    let f = tx_fields(&st);
    assert!(!f.transmitting && f.tx_freq_hz.is_none());
    assert!((f.tx_bitrate_bps - 333e3 * 188.0 / 204.0).abs() < 1e-6, "{f:?}");
}

/// The steady part of a transmission of null packets: after the rise and before the fall.
fn steady(c: &TxConfig, rate: f64, seconds: f64) -> Vec<Complex32> {
    let (x, _, _) = send(c, rate, &[], seconds, 0.0);
    x[(0.02 * rate) as usize..((seconds - 0.02) * rate) as usize].to_vec()
}

#[test]
fn the_spectrum_falls_off_outside_the_carrier() {
    for (rs, rate) in [(1e6, 8e6), (125e3, 2e6)] {
        let x = steady(&cfg(rs, PunctureRate::R23), rate, 0.5);
        let n = 4096;
        let mut psd = vec![0f32; n];
        welch(&x, n, Window::BlackmanHarris4, 0.5, Scaling::Power, &mut psd).unwrap();
        let bin = |f: f64| ((f / rate * n as f64).round() as isize + n as isize / 2) as usize;
        let band = |a: f64, b: f64| {
            let v: Vec<f64> = (bin(a)..=bin(b)).map(|i| f64::from(psd[i])).collect();
            v.iter().sum::<f64>() / v.len() as f64
        };
        let centre = band(-0.3 * rs, 0.3 * rs);
        // The shoulder at twice the symbol rate, 40 dB down (specification 7.8), and the power more than half the occupied
        // width beyond each edge of the occupied band, 50 dB down (specification 0.6).
        let db = |p: f64| 10.0 * (p.max(1e-30) / centre).log10();
        let shoulder = db(band(1.95 * rs, 2.05 * rs).max(band(-2.05 * rs, -1.95 * rs)));
        let far = (0.49 * rate).min(3.0 * rs);
        let beyond = db(band(1.35 * rs, far).max(band(-far, -1.35 * rs)));
        eprintln!("{rs} sym/s at {rate} sps: shoulder at 2 Rs {shoulder:.1} dB, beyond 1.35 Rs {beyond:.1} dB");
        assert!(shoulder < -40.0 && beyond < -50.0, "{rs}: {shoulder} {beyond}");
    }
}

#[test]
fn level_and_peaks() {
    let x = steady(&cfg(1e6, PunctureRate::R34), 4e6, 0.4);
    let n = x.len() as f64;
    let rms = (x.iter().map(|z| f64::from(z.norm_sqr())).sum::<f64>() / n).sqrt();
    let peak = x.iter().map(|z| z.norm()).fold(0f32, f32::max);
    eprintln!("rms {rms:.4}, peak {peak:.3}");
    assert!((rms - f64::from(RMS)).abs() < 0.01, "{rms}");
    assert!(peak < 1.0);
}

#[test]
fn stop_ends_with_a_fall_after_a_group_of_eight_packets() {
    let c = cfg(1e6, PunctureRate::R12);
    let rate = 2e6;
    let (mut st, host) = station(&c, rate);
    call(&mut st, "start", None).unwrap();
    run(&mut st, rate, 0.2, |_| {});
    call(&mut st, "stop", None).unwrap();
    assert!(matches!(
        refusal(&call(&mut st, "start", None).unwrap_err()),
        Some(TxError::Ending)
    ));
    run(&mut st, rate, 0.2, |_| {});
    assert!(!tx_fields(&st).transmitting);
    let (x, ends) = host.sent();
    assert_eq!(ends, 1);
    // The fall is a raised cosine of 5 ms to zero, after the end of the group of eight packets on air.
    let fall = (RISE_S * rate) as usize;
    let tail = &x[x.len() - fall..];
    let level = |s: &[Complex32]| (s.iter().map(|z| z.norm_sqr()).sum::<f32>() / s.len() as f32).sqrt();
    let before = level(&x[x.len() - 2 * fall..x.len() - fall]);
    assert!((before - RMS).abs() < 0.05, "{before}");
    assert!(level(&tail[..fall / 8]) > 0.8 * RMS, "the fall starts high");
    assert!(level(&tail[fall - fall / 16..]) < 0.02, "the fall ends at zero");
    let stopped_at = 0.2 * rate;
    let packet_s = 1504.0 / c.bit_rate();
    let most = stopped_at + 0.05 * rate + 8.0 * packet_s * rate + fall as f64;
    assert!((x.len() as f64) < most, "{} samples, at most {most}", x.len());
    // After the end the stack starts again.
    call(&mut st, "start", None).expect("a new transmission");
}

#[test]
fn start_needs_the_transmitter_and_a_rate_for_the_symbol_rate() {
    let (mut st, _host) = station(&cfg(1e6, PunctureRate::R23), 2e6);
    st.set_transmit(false);
    assert!(matches!(
        call(&mut st, "start", None),
        Err(CommandError::TransmitOff)
    ));
    st.set_transmit(true);
    let (mut st, _host) = station(&cfg(2e6, PunctureRate::R23), 2e6);
    assert!(matches!(
        refusal(&call(&mut st, "start", None).unwrap_err()),
        Some(TxError::RateTooLow { .. })
    ));
}

#[test]
fn append_takes_whole_packets_with_a_sync_byte_while_the_queue_has_room() {
    let (mut st, _host) = station(&cfg(1e6, PunctureRate::R23), 2e6);
    let full: Vec<u8> = host_stream(1e6, QUEUE_PACKETS).into_iter().flatten().collect();
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
    assert_eq!(tx_fields(&st).tx_queue, 0, "all or none");
    call(&mut st, "append", Some(&full)).expect("the queue takes 6000 packets");
    assert_eq!(tx_fields(&st).tx_queue as usize, QUEUE_PACKETS);
    let e = call(&mut st, "append", Some(&full[..TS_SIZE])).unwrap_err();
    assert_eq!(
        refusal(&e),
        Some(TxError::Queue(QueueError::Full {
            packets: 1,
            room: 0
        }))
    );
    let f = tx_fields(&st);
    let want = QUEUE_PACKETS as f64 * 1504.0 / f.tx_bitrate_bps;
    assert!((f.tx_queue_s.0 - want).abs() < 1e-9, "{f:?}");
}



#[test]
fn the_sender_runs_faster_than_real_time() {
    for b in sdrkit_stack_dvbcs::benchmarks()
        .iter()
        .filter(|b| b.name.contains("dvbs_sender"))
    {
        let r = (b.run)(0.2);
        eprintln!("{}: {:.1} % of one core", b.name, 100.0 * r.cpu_fraction_of_realtime);
        assert!(r.cpu_fraction_of_realtime > 0.0 && r.cpu_fraction_of_realtime < 1.0, "{r:?}");
    }
}

