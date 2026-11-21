//! Lightning position: the datagrams, the replay guard and the rate limit; the sferic timer; the whole chain from the IQ of six
//! stations to the server; the test signal with a timing transmitter; pieces of any size; the commands.

use sci::family::TEST_START_UTC;
use sci::lightning::gen::{add_stroke, stroke_hz, TdoaSignal};
use sci::lightning::packet::{
    mac_key, station_id, Body, Detection, Freshness, Packet, PacketError, RateLimit,
    ReplayGuard, StationHeader, TimeSource, MAGIC, VERSION,
};
use sci::lightning::server::{Server, ServerConfig};
use sci::lightning::station::{HostTime, Station, StationConfig, TimingTx};
use sci::lightning::timer::SfericTimer;
use sci::lightning::{
    DESCRIPTOR, GEN_TIMING_TX, NETWORK_KEY, ROLE, START_UTC, THRESHOLD, TIMING_HZ, TIMING_ID,
    TIMING_LAT, TIMING_LON,
};
use sci::typed::{
    LightningTdoaDatagramArgs, LightningTdoaFields, LightningTdoaStrikeRow,
    LightningTdoaTimeMarkArgs, LIGHTNING_STRIKE, LIGHTNING_TDOA_STATIONS, LIGHTNING_TDOA_STRIKES,
};
use sdrkit_core::interface::Arguments;
use sdrkit_algo_locate::clock::Mark;
use sdrkit_algo_locate::propagation::{destination, distance_m};
use sdrkit_algo_locate::Propagation;
use sdrkit_algo_sim::noise::Awgn;
use sdrkit_algo_sim::rng::Rng;
use sdrkit_core::{Complex32, InputInfo, Output, Receiver, Record, Value};
use sdrkit_dev_test::Harness;
use sdrkit_stack_science as sci;
use sdrkit_util_geo::Position;
use std::f64::consts::TAU;
use std::sync::Arc;

fn five() -> Packet {
    let d: Vec<Detection> = (0..5)
        .map(|k| Detection {
            utc_ns: 1_767_225_600_123_456_789 + k * 1_000_003,
            sigma_ns: 850 + k as u32,
            peak_db: 23.45,
            amplitude: 0.125,
            width_us: 37.3,
            rise_us: 6.1,
            mean_hz: -1230.0,
        })
        .collect();
    Packet {
        station: station_id("north-ridge"),
        seq: 4711,
        sent_ns: 1_767_225_601_000_000_000,
        header: StationHeader {
            lat: 54.6872,
            lon: -3.2786,
            alt_m: 120,
            source: Some(TimeSource::Gnss),
            position_by_user: true,
            common_view: false,
            clock_sigma_ns: 300,
        },
        body: Body::Detections(d),
    }
}

#[test]
fn five_detections_round_trip() {
    let key = mac_key("a network key of some length");
    let p = five();
    let b = p.encode(&key);
    assert_eq!(b.len(), 215);
    let q = Packet::decode(&b, &key).expect("decodes");
    assert_eq!(q.station, p.station);
    assert_eq!(q.seq, 4711);
    assert_eq!(q.header.source, Some(TimeSource::Gnss));
    assert!((q.header.lat - 54.6872).abs() < 1e-6);
    let (Body::Detections(a), Body::Detections(b2)) = (&p.body, &q.body) else {
        panic!("detections expected");
    };
    for (x, y) in a.iter().zip(b2) {
        assert_eq!(x.utc_ns, y.utc_ns);
        assert!((x.width_us - y.width_us).abs() <= 0.06);
    }
}

#[test]
fn wrong_key_changed_byte_short_and_random_are_refused() {
    let key = mac_key("a network key of some length");
    let b = five().encode(&key);
    assert_eq!(
        Packet::decode(&b, &mac_key("another key of some length")),
        Err(PacketError::Mac)
    );
    let mut c = b.clone();
    c[60] ^= 1;
    assert_eq!(Packet::decode(&c, &key), Err(PacketError::Mac));
    assert_eq!(Packet::decode(&b[..40], &key), Err(PacketError::Short));
    let mut rng = Rng::new(7);
    let mut accepted = 0;
    for _ in 0..2000 {
        let n = 55 + (rng.uniform() * 300.0) as usize;
        let mut r: Vec<u8> = (0..n).map(|_| (rng.uniform() * 256.0) as u8).collect();
        r[..4].copy_from_slice(&MAGIC);
        r[4] = VERSION;
        if Packet::decode(&r, &key).is_ok() {
            accepted += 1;
        }
    }
    assert_eq!(accepted, 0);
}

#[test]
fn replay_guard() {
    let now = 1_767_225_600_000_000_000i64;
    let mut g = ReplayGuard::new(600.0);
    let s = station_id("east");
    assert_eq!(g.check(s, 100, now, now), Freshness::Fresh);
    assert_eq!(g.check(s, 100, now, now), Freshness::Replay);
    assert_eq!(g.check(s, 99, now, now), Freshness::Fresh);
    assert_eq!(g.check(s, 99, now, now), Freshness::Replay);
    assert_eq!(
        g.check(s, 101, now - 700_000_000_000, now),
        Freshness::Stale
    );
    assert_eq!(g.check(s, 10, now, now), Freshness::Stale);
    assert_eq!(
        g.check(s, 10, now + 5_000_000_000, now),
        Freshness::Fresh
    );
}

#[test]
fn rate_limit_and_station_id() {
    let mut r = RateLimit::default();
    let passed = (0..1000).filter(|_| r.allow("10.0.0.1", 0.0)).count();
    assert_eq!(passed, 400);
    assert!(r.allow("10.0.0.1", 0.01));
    assert_eq!(station_id(""), 2_166_136_261);
    assert_ne!(station_id("a"), station_id("b"));
}

/// The edge minus the onset of 40 strokes at one peak SNR, µs, or `None` for a stroke not found.
fn timer_run(peak_db: f64, seed: u64) -> Vec<Option<f64>> {
    const RATE: f64 = 500e3;
    const NOISE: f64 = 0.004;
    let true_rate = RATE * (1.0 + 12e-6);
    let station = Position::new(52.5, 13.4);
    let strike = Position::new(52.5, 14.3);
    let delay = Propagation::default().delay_s(&strike, &station);
    let mut rng = Rng::new(seed);
    let onsets: Vec<f64> = (0..40)
        .map(|k| (0.5 + 0.05 * k as f64 + 0.01 * rng.uniform() + delay) * true_rate)
        .collect();
    let n = (2.7 * RATE) as usize;
    let mut x = vec![Complex32::default(); n];
    Awgn::with_power(NOISE * NOISE, seed + 1)
        .unwrap()
        .process(&mut x);
    let amp = NOISE * 10f64.powf(peak_db / 20.0);
    for &o in &onsets {
        add_stroke(&mut x, 0, true_rate, o, amp, stroke_hz(RATE), TAU * rng.uniform());
    }
    let mut timer = SfericTimer::new(RATE, 12.0, None).unwrap();
    let mut pulses = Vec::new();
    for (k, c) in x.chunks(10_000).enumerate() {
        timer.process(c, (k * 10_000) as u64, &mut pulses);
    }
    timer.flush(&mut pulses);
    onsets
        .iter()
        .map(|&o| {
            pulses
                .iter()
                .map(|p| (p.edge - o) / RATE * 1e6)
                .find(|d| d.abs() < 50.0)
        })
        .collect()
}

fn mean_std(v: &[f64]) -> (f64, f64) {
    let m = v.iter().sum::<f64>() / v.len() as f64;
    let s = (v.iter().map(|x| (x - m) * (x - m)).sum::<f64>() / v.len() as f64).sqrt();
    (m, s)
}

#[test]
fn the_sferic_timer_finds_every_stroke_and_times_its_edge() {
    let mut biases = Vec::new();
    for (k, level) in [14.0, 22.0, 34.0, 46.0].into_iter().enumerate() {
        let d = timer_run(level, 10 + k as u64);
        let found: Vec<f64> = d.iter().flatten().copied().collect();
        assert_eq!(found.len(), 40, "{level} dB: {} of 40", found.len());
        let (mean, std) = mean_std(&found);
        assert!(
            (2.0..9.0).contains(&mean),
            "{level} dB: edge {mean:.2} µs after the onset"
        );
        if level > 25.0 {
            assert!(std < 0.4, "{level} dB: scatter {std:.3} µs");
        }
        biases.push(mean);
    }
    let spread = biases.iter().copied().fold(f64::MIN, f64::max)
        - biases.iter().copied().fold(f64::MAX, f64::min);
    assert!(spread < 0.5, "edge bias over the levels {biases:?}");
}

/// Strokes in 8-bit samples whose noise rounds to zero: the floor of digital silence is held at the detector's least value, so it
/// is valid and every one of 14 strokes is timed.
#[test]
fn the_sferic_timer_times_strokes_in_8_bit_silence() {
    const RATE: f64 = 500e3;
    let mut rng = Rng::new(5);
    let onsets: Vec<f64> = (0..14)
        .map(|k| (0.5 + 0.1 * k as f64 + 0.01 * rng.uniform()) * RATE)
        .collect();
    let mut x = vec![Complex32::default(); (2.2 * RATE) as usize];
    // Noise of a tenth of one step of 8 bits rounds to zero.
    Awgn::with_power((0.1f64 / 127.0).powi(2), 6)
        .unwrap()
        .process(&mut x);
    for &o in &onsets {
        add_stroke(&mut x, 0, RATE, o, 0.5, stroke_hz(RATE), TAU * rng.uniform());
    }
    let q = |v: f32| (v * 127.0).round().clamp(-127.0, 127.0) / 127.0;
    for z in &mut x {
        *z = Complex32::new(q(z.re), q(z.im));
    }
    let silent = x.iter().filter(|z| z.norm_sqr() == 0.0).count();
    assert!(silent * 10 > x.len() * 9, "{silent} of {} samples are zero", x.len());
    let mut timer = SfericTimer::new(RATE, 12.0, None).unwrap();
    let mut pulses = Vec::new();
    for (k, c) in x.chunks(10_000).enumerate() {
        timer.process(c, (k * 10_000) as u64, &mut pulses);
    }
    timer.flush(&mut pulses);
    assert!(timer.is_valid());
    assert!(timer.floor().is_some_and(|f| f > 0.0), "{:?}", timer.floor());
    let timed = onsets
        .iter()
        .filter(|&&o| pulses.iter().any(|p| ((p.edge - o) / RATE * 1e6).abs() < 50.0))
        .count();
    assert_eq!(timed, 14, "{} pulses", pulses.len());
}

const T0_NS: i64 = 1_767_225_600_000_000_000;
const KEY: &str = "a network key of some length";
const RATE: f64 = 250e3;

/// How a station of the chain keeps time.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Keeps {
    Gps,
    Gnss,
    /// Common view, with a computer clock this far off, s.
    CommonView(f64),
    /// NTP only, this far off, s.
    Ntp(f64),
}

fn timing_tx() -> TimingTx {
    TimingTx {
        id: 7,
        label: "test".into(),
        freq_hz: 150e3,
        baud: 200.0,
        position: Position::new(52.0, 10.0),
    }
}

/// The datagrams of the chain with their arrival times (s) and senders, and the count of duplicates among them.
struct Network {
    rng: Rng,
    datagrams: Vec<(f64, usize, Vec<u8>)>,
    duplicates: usize,
}

/// Station `k` of the chain: its station and its signal. Its datagrams go to `net`, 15 % of them twice, each after a random
/// delay of up to 0.3 s.
fn chain_station(
    k: usize,
    pos: Position,
    (keeps, ppm): (Keeps, f64),
    seconds: f64,
    net: &mut Network,
) -> (Station, TdoaSignal) {
    let rng = &mut net.rng;
    let timing = !matches!(keeps, Keeps::Ntp(_));
    let mut sig = TdoaSignal::new(RATE, pos, 2.0, 6.0, 77)
        .at_start(0.0137 * k as f64)
        .with_clock_ppm(ppm);
    if timing {
        sig = sig
            .with_timing(timing_tx().position, 50e3, 200.0, 0.05, 900 + k as u64)
            .unwrap();
    }
    let mut st = Station::new(StationConfig {
        rate: RATE,
        threshold_db: 12.0,
        timing: timing.then(|| (timing_tx(), 50e3)),
        host: HostTime::None,
    })
    .unwrap();
    st.set_name(Some(&format!("st{k}")));
    st.set_position(Some(pos), 0.0);
    let mut noise = Awgn::with_power(1e-4, 500 + k as u64).unwrap();
    let chunk = (RATE / 4.0) as usize;
    let mut buf = vec![Complex32::default(); chunk];
    let mut dg = Vec::new();
    for c in 0..(seconds * 4.0) as u64 {
        let first = c * chunk as u64;
        let utc = |s: &TdoaSignal, n: u64| T0_NS + (s.true_time(n as f64) * 1e9).round() as i64;
        let mark = match keeps {
            Keeps::Gps if c % 4 == 0 => {
                Some(st.gps_mark(first, utc(&sig, first) + (20.0 * rng.normal()) as i64))
            }
            Keeps::Gnss if c % 4 == 0 => Some(st.gnss_mark(
                first,
                utc(&sig, first) + (300.0 * rng.normal()) as i64,
                300e-9,
                0.0,
            )),
            Keeps::CommonView(off) | Keeps::Ntp(off) if c % 2 == 0 => Some(Mark {
                source: TimeSource::Ntp,
                sample_s: first as f64 / RATE,
                utc_ns: utc(&sig, first) + (off * 1e9) as i64,
                sigma_s: 1e-6,
                bias_s: TimeSource::Ntp.typical_sigma_s(),
            }),
            _ => None,
        };
        if let Some(m) = mark {
            st.mark(m).unwrap();
        }
        buf.fill(Complex32::default());
        noise.process(&mut buf);
        sig.add(&mut buf);
        st.process(&buf, first);
        dg.clear();
        let now = st.now_ns().unwrap();
        st.datagrams(now, KEY, &mut dg);
        let sent = sig.true_time((first + chunk as u64) as f64);
        for d in dg.drain(..) {
            if rng.uniform() < 0.15 {
                net.datagrams.push((sent + 0.3 * rng.uniform(), k, d.clone()));
                net.duplicates += 1;
            }
            net.datagrams.push((sent + 0.3 * rng.uniform(), k, d));
        }
    }
    (st, sig)
}

/// The seconds of the chain.
const CHAIN_S: f64 = 15.5;
/// How the six stations of the chain keep time, and their clock errors (ppm).
const SITES: [(Keeps, f64); 6] = [
    (Keeps::Gps, 3.1),
    (Keeps::Gps, -5.4),
    (Keeps::Gnss, 11.2),
    (Keeps::CommonView(1.1e-3), -8.7),
    (Keeps::CommonView(7.3e-3), 4.4),
    (Keeps::Ntp(2e-3), -2.0),
];

/// The chain: 15.5 s at 250 ksps for six stations on a 300 km circle around 52.5 N 13.4 E; strikes from 6 s on, when the
/// common-view clocks are fine. The stations with their signals, and the datagrams in order of arrival.
fn chain() -> (Vec<(Station, TdoaSignal)>, Network) {
    let centre = Position::new(52.5, 13.4);
    let mut net = Network {
        rng: Rng::new(31),
        datagrams: Vec::new(),
        duplicates: 0,
    };
    let mut stations = Vec::new();
    for (k, &site) in SITES.iter().enumerate() {
        let pos = destination(&centre, 60.0 * k as f64 + 10.0, 300e3);
        stations.push(chain_station(k, pos, site, CHAIN_S, &mut net));
    }
    net.datagrams.sort_by(|a, b| a.0.total_cmp(&b.0));
    (stations, net)
}

/// The errors (m) of the strikes of `heard` that `rows` (time UTC ns, latitude, longitude) located within 20 µs and 5 km, and
/// the count of strikes that the chain can locate (those before its last second).
fn located(sig: &TdoaSignal, rows: &[(i64, f64, f64)]) -> (Vec<f64>, usize) {
    let truth: Vec<_> = sig
        .heard()
        .iter()
        .filter(|h| h.strike.time_s < CHAIN_S - 1.0)
        .map(|h| h.strike)
        .collect();
    let mut errors = Vec::new();
    for s in &truth {
        let t_ns = T0_NS + (s.time_s * 1e9).round() as i64;
        let best = rows
            .iter()
            .filter(|r| (r.0 - t_ns).abs() < 20_000)
            .map(|r| distance_m(&Position::new(r.1, r.2), &s.position))
            .fold(f64::INFINITY, f64::min);
        if best < 5_000.0 {
            errors.push(best);
        }
    }
    errors.sort_by(f64::total_cmp);
    (errors, truth.len())
}

#[test]
fn the_whole_chain_from_iq_locates_the_strikes() {
    let (stations, net) = chain();
    let mut server = Server::new(ServerConfig {
        network_key: KEY.into(),
        propagation: Propagation::default(),
        min_stations: 4,
        timing: Some(timing_tx()),
    })
    .unwrap();
    let mut i = 0;
    let mut t = 0.0;
    while t < CHAIN_S + 3.5 {
        t += 0.1;
        let now = T0_NS + (t * 1e9) as i64;
        while i < net.datagrams.len() && net.datagrams[i].0 <= t {
            let (_, k, d) = &net.datagrams[i];
            let _ = server.datagram(d, &format!("10.0.0.{k}"), now);
            i += 1;
        }
        server.run(now);
    }
    let c = server.counters();
    assert!(net.duplicates > 0);
    assert_eq!(c.replays, net.duplicates as u64, "{c:?}");
    assert_eq!((c.mac, c.malformed, c.stale), (0, 0, 0), "{c:?}");

    let rows = server.strikes();
    let fixes: Vec<(i64, f64, f64)> = rows.iter().map(|r| (r.utc_ns, r.lat, r.lon)).collect();
    let (errors, strikes) = located(&stations[0].1, &fixes);
    assert!(
        errors.len() * 10 >= strikes * 8,
        "{} of {strikes} located; {:?}",
        errors.len(),
        server.match_counters()
    );
    let median = errors[errors.len() / 2];
    assert!(median < 1_000.0, "median {median:.0} m; {errors:?}");
    assert!(rows.iter().all(|r| r.utc));

    // The NTP station is only also heard; the station 7.3 ms off takes part with its common-view correction.
    let index = |name: &str| {
        server
            .stations()
            .iter()
            .position(|s| s.id == station_id(name))
            .unwrap()
    };
    let ntp = index("st5");
    assert!(rows.iter().all(|r| !r.used.contains(&ntp)));
    assert!(rows.iter().any(|r| r.heard.contains(&ntp)));
    let off = &server.stations()[index("st4")];
    assert!(off.used > 0 && off.tied_to_utc);
    let c = off.correction_s.unwrap();
    assert!((c + 2.3e-3).abs() < 20e-6, "correction {c}");

    // Each fine station measured its clock's drift.
    for (k, &(_, ppm)) in SITES.iter().enumerate().take(5) {
        let st = &stations[k].0;
        let (src, _) = st.now_stamp().unwrap();
        assert!(src.fine_capable(), "station {k}: {src:?}");
        let got = st.clocks().clock(src).ppm().unwrap();
        assert!((got - ppm).abs() < 0.5, "station {k}: {got} ppm against {ppm}");
    }
}

#[test]
fn a_server_receiver_fed_through_the_datagram_command_publishes_the_strikes() {
    let (stations, net) = chain();
    let h = Harness::new(&DESCRIPTOR)
        .rate(RATE)
        .set(&ROLE, sci::lightning::Role::Server)
        .set(&NETWORK_KEY, KEY)
        .set(&START_UTC, T0_NS as f64 * 1e-9)
        .set(&TIMING_ID, 7)
        .set(&TIMING_HZ, 150e3)
        .set(&TIMING_LAT, 52.0)
        .set(&TIMING_LON, 10.0);
    let mut rx = h.receiver().unwrap();
    rx.configure(&InputInfo::new(RATE, 100e3, 100e3).recording())
        .unwrap();
    let mut noise = Awgn::with_power(1e-4, 77).unwrap();
    let mut x = vec![Complex32::default(); (RATE / 10.0) as usize];
    let mut out = Output::default();
    let mut events = 0;
    let mut i = 0;
    let mut t = 0.0;
    while t < CHAIN_S + 3.5 {
        while i < net.datagrams.len() && net.datagrams[i].0 <= t {
            let b: Arc<[u8]> = Arc::from(net.datagrams[i].2.as_slice());
            let _ = command(rx.as_mut(), "datagram", |a| {
                LightningTdoaDatagramArgs { bytes: b }.write(a);
            });
            i += 1;
        }
        x.fill(Complex32::default());
        noise.process(&mut x);
        out.clear();
        rx.feed(&x, &mut out);
        events += out.events.iter().filter(|e| e.is(&LIGHTNING_STRIKE)).count();
        t += 0.1;
    }
    let r = rx.results();
    let f = r.fields();
    assert_eq!(f.at(&LightningTdoaFields::REPLAYS), Some(net.duplicates as i64));
    let table = r.table(r.decl().table_id(LIGHTNING_TDOA_STRIKES.key).unwrap()).unwrap();
    let fixes: Vec<(i64, f64, f64)> = table
        .rows_newest_first()
        .map(|(_, row)| {
            (
                row.at(&LightningTdoaStrikeRow::UTC_NS).unwrap(),
                row.at(&LightningTdoaStrikeRow::LAT).unwrap(),
                row.at(&LightningTdoaStrikeRow::LON).unwrap(),
            )
        })
        .collect();
    let (errors, strikes) = located(&stations[0].1, &fixes);
    assert!(errors.len() * 10 >= strikes * 8, "{} of {strikes}", errors.len());
    assert_eq!(f.at(&LightningTdoaFields::FIXES), Some(fixes.len() as i64));
    assert_eq!(events, fixes.len());
    assert_eq!(r.points().len(), fixes.len());
    let st = r.table(r.decl().table_id(LIGHTNING_TDOA_STATIONS.key).unwrap()).unwrap();
    assert_eq!(st.len(), 6);
}

#[test]
fn the_test_signal_with_the_timing_transmitter_and_a_7_ppm_clock() {
    let r = Harness::new(&DESCRIPTOR)
        .rate(250e3)
        .seconds(10.0)
        .set(&GEN_TIMING_TX, true)
        .set(&TIMING_HZ, 150e3)
        .set(&TIMING_LAT, 52.0)
        .set(&TIMING_LON, 10.0)
        .set(&sdrkit_algo_sim::chain::GEN_CLOCK_PPM, 7.0)
        .set(
            &sdrkit_algo_sim::chain::GEN_NOISE,
            sdrkit_algo_sim::chain::NoiseKind::Snr,
        )
        .set(&sdrkit_algo_sim::chain::GEN_SNR_DB, 30.0)
        .run()
        .unwrap();
    let f = r.results.fields();
    let pulses = f.at(&LightningTdoaFields::PULSES).unwrap();
    assert!(pulses >= 12, "{pulses} pulses; {}", r.results.status().summary);
    assert_eq!(f.at(&LightningTdoaFields::COMMON_VIEW), Some(true));
    assert_eq!(f.at(&LightningTdoaFields::TIMING_LOCKED), Some(true));
    let ppm = r.field("clock_ppm").cloned();
    let Some(Value::F64(ppm)) = ppm else {
        panic!("no drift: {ppm:?}");
    };
    assert!((ppm - 7.0).abs() < 1.0, "drift {ppm} ppm");
}

/// A station of the test signal with the timing transmitter, fed `x` in pieces of the sizes `sizes` (cycled).
fn pieces(x: &[Complex32], sizes: &[usize]) -> Station {
    let mut st = Station::new(StationConfig {
        rate: RATE,
        threshold_db: 12.0,
        timing: Some((timing_tx(), 50e3)),
        host: HostTime::File {
            start_utc_ns: T0_NS,
            source: TimeSource::Ntp,
            bias_s: 5e-3,
        },
    })
    .unwrap();
    let mut i = 0;
    for &n in sizes.iter().cycle() {
        if i >= x.len() {
            break;
        }
        let k = n.min(x.len() - i);
        st.process(&x[i..i + k], 1_000 + i as u64);
        i += k;
    }
    st.flush();
    st
}

#[test]
fn pieces_of_any_size_give_the_same_detections() {
    let mut sig = TdoaSignal::new(RATE, Position::new(52.5, 13.4), 3.0, 0.5, 5)
        .with_timing(timing_tx().position, 50e3, 200.0, 0.05, 6)
        .unwrap();
    let mut x = vec![Complex32::default(); (6.5 * RATE) as usize];
    Awgn::with_power(1e-4, 8).unwrap().process(&mut x);
    sig.add(&mut x);
    let a = pieces(&x, &[x.len()]);
    let b = pieces(&x, &[1, 7, 999, 4096, 33, 20_000, 65_536]);
    assert!(a.pulses() >= 10, "{} pulses", a.pulses());
    assert!(!a.readings().is_empty());
    assert_eq!(a.pending(), b.pending());
    assert_eq!(a.readings(), b.readings());
    assert_eq!(a.pulses(), b.pulses());
}

/// Runs command `key` with the arguments that `set` writes; whether it was accepted.
fn command(rx: &mut dyn Receiver, key: &str, set: impl FnOnce(&mut Record)) -> bool {
    let id = rx.commands().id(key).unwrap();
    let mut args = rx.commands().def(id).unwrap().new_args();
    set(&mut args);
    sdrkit_core::owner::invoke(rx, id, &args).is_ok()
}

fn feed(rx: &mut dyn Receiver, x: &[Complex32]) {
    let mut out = Output::default();
    for c in x.chunks(25_000) {
        out.clear();
        rx.feed(c, &mut out);
    }
}

#[test]
fn clear_and_reset_keep_the_clock_and_the_settings() {
    let h = Harness::new(&DESCRIPTOR)
        .rate(RATE)
        .seconds(6.5)
        .set(&GEN_TIMING_TX, true)
        .set(&TIMING_HZ, 150e3)
        .set(&TIMING_LAT, 52.0)
        .set(&TIMING_LON, 10.0);
    let x = h.generate().unwrap();
    let mut rx = h.receiver().unwrap();
    rx.configure(&InputInfo::new(RATE, 100e3, 100e3).recording())
        .unwrap();
    feed(rx.as_mut(), &x);
    let f = rx.results().fields();
    assert!(f.at(&LightningTdoaFields::PULSES).unwrap() > 0);
    assert_eq!(f.at(&LightningTdoaFields::TIME_SOURCE), Some("common view"));
    assert!(command(rx.as_mut(), "clear", |_| {}));
    let f = rx.results().fields();
    assert_eq!(f.at(&LightningTdoaFields::PULSES), Some(0));
    assert_eq!(f.at(&LightningTdoaFields::TIME_SOURCE), Some("common view"));
    let id = rx.schema().id(THRESHOLD.key()).unwrap();
    sdrkit_core::owner::apply(rx.as_mut(), id, &Value::F64(15.0)).unwrap();
    assert!(command(rx.as_mut(), "reset", |_| {}));
    assert_eq!(rx.values().unwrap().get(&THRESHOLD), 15.0);
    let f = rx.results().fields();
    assert_eq!(f.at(&LightningTdoaFields::PULSES), Some(0));
    assert_eq!(f.at(&LightningTdoaFields::TIME_SOURCE), Some("common view"));
}

#[test]
fn the_server_takes_datagrams_and_time_marks_through_commands() {
    let h = Harness::new(&DESCRIPTOR)
        .rate(RATE)
        .set(&ROLE, sci::lightning::Role::Server)
        .set(&NETWORK_KEY, KEY)
        .set(&START_UTC, TEST_START_UTC);
    let mut rx = h.receiver().unwrap();
    rx.configure(&InputInfo::new(RATE, 100e3, 100e3).recording())
        .unwrap();
    let mut x = vec![Complex32::default(); (RATE * 0.5) as usize];
    Awgn::with_power(1e-4, 3).unwrap().process(&mut x);
    feed(rx.as_mut(), &x);
    let hello = Packet {
        station: station_id("remote"),
        seq: 9,
        sent_ns: (TEST_START_UTC * 1e9) as i64,
        header: StationHeader {
            lat: 52.0,
            lon: 13.0,
            source: Some(TimeSource::Gps),
            clock_sigma_ns: 100,
            ..StationHeader::default()
        },
        body: Body::Hello("remote".into()),
    }
    .encode(&mac_key(KEY));
    let datagram = |b: &[u8]| {
        let b: Arc<[u8]> = Arc::from(b);
        move |a: &mut Record| {
            LightningTdoaDatagramArgs { bytes: b }.write(a);
        }
    };
    assert!(command(rx.as_mut(), "datagram", datagram(&hello)));
    assert!(!command(rx.as_mut(), "datagram", datagram(&hello)));
    assert!(!command(rx.as_mut(), "datagram", datagram(&[0u8; 80])));
    feed(rx.as_mut(), &x[..25_000]);
    let f = rx.results().fields();
    assert_eq!(f.at(&LightningTdoaFields::PACKETS_OK), Some(1));
    assert_eq!(f.at(&LightningTdoaFields::REPLAYS), Some(1));
    // Two GPS marks from the host make GPS the time source.
    let utc = (TEST_START_UTC * 1e9) as i64;
    for k in 0..2i64 {
        assert!(command(rx.as_mut(), "time_mark", |a| {
            LightningTdoaTimeMarkArgs {
                source: "gps",
                sample: k * 62_500,
                utc_ns: utc + k * 250_000_000,
                sigma_ns: None,
                bias_ns: None,
            }
            .write(a);
        }));
    }
    assert_eq!(rx.results().fields().at(&LightningTdoaFields::TIME_SOURCE), Some("GPS"));
}
