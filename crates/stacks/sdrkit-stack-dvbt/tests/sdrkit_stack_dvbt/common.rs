//! The round-trip harness of the acceptance tests: the generator with the counted test pattern, impairments from
//! `sdrkit-dev-test` (independent of the generator), the receiver through its public interface, and the packet check.
#![allow(dead_code)]

use sdrkit_algo_dsp::resample::FractionalResampler;
use sdrkit_core::owner::Owner;
use sdrkit_core::{
    Choice, Complex32, InputInfo, LockState, Output, Receiver, Results, SettingSet, StackConfig,
    Value, Values,
};
use sdrkit_dev_test::{Awgn, ClockError, Echo, FrequencyOffset, Quantizer};
use sdrkit_graph::{Data, StackGraph, TapReader, Timed};
use sdrkit_proto_mpegts::source::{Fill, Numbered, NumberedSource};
use sdrkit_proto_mpegts::stream::TsPacket;
use sdrkit_stack_dvbt::generator::{DvbtGenerator, GenParams};
use sdrkit_stack_dvbt::generator::{PATTERN_HP, PATTERN_LP};
use sdrkit_stack_dvbt::params::{Bandwidth, TpsParams};
use sdrkit_stack_dvbt::settings::{BandwidthChoice, DvbtSetting, StreamChoice};
use sdrkit_stack_dvbt::typed::DvbtFields as F;
use sdrkit_stack_dvbt::typed::DvbtFields;

/// The configuration of the DVB-T stack with these settings, by key.
pub fn config(settings: &[(&str, Value)]) -> StackConfig {
    let schema = DvbtSetting::schema();
    let mut v = Values::defaults(&schema);
    for (k, x) in settings {
        let id = schema.id(k).unwrap();
        v.set(id, x).unwrap();
    }
    StackConfig::new(v)
}

/// The value of a choice setting.
pub fn choice<C: Choice>(c: C) -> Value {
    Value::Choice(c.index())
}

/// The data folder of the repository.
fn data() -> sdrkit_presets::Data {
    sdrkit_presets::Data::open(concat!(env!("CARGO_MANIFEST_DIR"), "/../../../data")).unwrap()
}

/// The `dvbt` settings that the data folder gives the country `code`, the bandwidth as a choice.
pub fn country_settings(code: &str) -> Vec<(String, Value)> {
    data()
        .dvbt_settings(code)
        .unwrap_or_else(|| panic!("{code}: no DVB-T plan"))
        .into_iter()
        .map(|(k, v)| match (k.as_str(), &v) {
            ("bandwidth", Value::Text(t)) => {
                let c = BandwidthChoice::from_key(t).unwrap();
                (k, choice(c))
            }
            _ => (k, v),
        })
        .collect()
}

/// The DVB-T channels of the country `code` in the data folder: label, centre frequency and width.
pub fn country_channels(code: &str) -> Vec<(String, f64, Option<Bandwidth>)> {
    data()
        .tv_channels("dvbt", code)
        .into_iter()
        .map(|c| {
            let width = c
                .width_hz
                .and_then(|w| Bandwidth::from_mhz((w / 1e6).round() as u32));
            (c.label, c.frequency_hz, width)
        })
        .collect()
}

/// A channel scan of `channels`.
pub fn channel_scan(
    channels: &[(String, f64, Option<Bandwidth>)],
    return_hz: f64,
) -> sdrkit_stack_dvbt::scan::ChannelScan {
    let list: Vec<(&str, f64, Option<Bandwidth>)> =
        channels.iter().map(|c| (c.0.as_str(), c.1, c.2)).collect();
    sdrkit_stack_dvbt::scan::ChannelScan::channels(&list, return_hz)
}

/// The value of the field `key` of `r`.
pub fn value<'a>(r: &'a Results, key: &str) -> Option<&'a Value> {
    r.fields().value(r.decl().field_id(key)?)
}

/// The DVB-T receiver of these settings.
pub fn stack(settings: &[(&str, Value)]) -> StackGraph {
    sdrkit_stack_dvbt::receiver::receiver(&config(settings)).unwrap()
}

/// Plays the service whose `services` row key is `row`.
pub fn select(rx: &mut StackGraph, row: &str) {
    let id = rx.commands().id("service.select").expect("the command");
    let mut args = rx.commands().def(id).expect("declared").new_args();
    args.set(&sdrkit_proto_mpegts::publish::SERVICE_ROW.field, row);
    sdrkit_core::owner::invoke(rx, id, &args).expect("selected");
}

/// The rows of table `key` of `r`.
pub fn rows(r: &Results, key: &str) -> usize {
    r.decl()
        .table_id(key)
        .and_then(|id| r.table(id))
        .map_or(0, |t| t.len())
}

/// Whether table `key` of `r` has the row `row`.
pub fn has_row(r: &Results, key: &str, row: &str) -> bool {
    r.decl()
        .table_id(key)
        .and_then(|id| r.table(id))
        .is_some_and(|t| t.row(row).is_some())
}

/// Every transport packet that a tap of `fec.out` holds, appended to `out`.
pub fn drain_packets(
    tap: &mut TapReader,
    buf: &mut sdrkit_graph::Buffer,
    out: &mut Vec<[u8; 188]>,
) {
    while tap.pop(buf) {
        if let Data::Items(Some(v)) = &buf.data {
            if let Some(items) = v.as_any().downcast_ref::<Vec<Timed<TsPacket>>>() {
                out.extend(items.iter().map(|p| *p.value.bytes()));
            }
        }
    }
}

/// One case.
#[derive(Clone, Debug)]
pub struct Case {
    pub bw: Bandwidth,
    pub tps: TpsParams,
    pub frames: usize,
    /// SNR in the K-carrier band. `None` for no noise.
    pub snr_db: Option<f64>,
    pub cfo_hz: f64,
    pub clock_ppm: f64,
    /// Echo: delay in samples, level in dB, Doppler of the echo in Hz.
    pub echo: Option<(usize, f64, f64)>,
    /// Input rate (resampled from the natural rate).
    pub input_rate: Option<f64>,
    /// Decode the LP stream.
    pub lp: bool,
    pub puncture_phase: usize,
    /// Samples of lead-in (noise, or silence when there is no noise).
    pub lead: usize,
    /// Start this far (fraction of a symbol) into the first symbol.
    pub start_frac: f64,
    /// I and Q swapped.
    pub swap_iq: bool,
    /// DC offset: level in dBc and phase.
    pub dc: Option<(f64, f64)>,
    /// IQ imbalance: gain dB, phase degrees.
    pub iq_imbalance: Option<(f64, f64)>,
    /// Non-finite samples (count) at this fraction of the signal.
    pub nan: Option<(usize, f64)>,
    /// Samples removed at this time (s).
    pub drop: Option<(f64, usize)>,
    /// A noise burst: start (s), duration (s), SNR during it.
    pub burst: Option<(f64, f64, f64)>,
    /// Undecodable noise level for the first seconds: (seconds, SNR).
    pub bad_start: Option<(f64, f64)>,
    /// 8-bit clipping at this rms fraction of full scale.
    pub clip8: Option<f64>,
    /// Feeding block size.
    pub block: usize,
    /// The stage boundaries that are cut, each with a thread: bit `i` is the boundary before stage `i + 1`.
    pub cuts: u32,
    /// The receiver's bandwidth setting. `None` is the width of the signal.
    pub rx_bandwidth: Option<BandwidthChoice>,
    /// Extra receiver settings.
    pub settings: Vec<(&'static str, Value)>,
    pub seed: u64,
    /// A raw LP code-rate field in TPS.
    pub lp_field: Option<u8>,
}

impl Case {
    pub fn new(bw: Bandwidth, tps: TpsParams) -> Self {
        Case {
            bw,
            tps,
            frames: if tps.mode == sdrkit_stack_dvbt::Mode::K8 {
                8
            } else {
                20
            },
            snr_db: Some(30.0),
            cfo_hz: 0.0,
            clock_ppm: 0.0,
            echo: None,
            input_rate: None,
            lp: false,
            puncture_phase: 0,
            lead: 40_000,
            start_frac: 0.0,
            swap_iq: false,
            dc: None,
            iq_imbalance: None,
            nan: None,
            drop: None,
            burst: None,
            bad_start: None,
            clip8: None,
            block: 16384,
            cuts: 0,
            rx_bandwidth: None,
            settings: Vec::new(),
            seed: 7,
            lp_field: None,
        }
    }
}

/// What came out.
#[derive(Debug, Default)]
pub struct Outcome {
    pub params: Option<String>,
    pub tps_ok: bool,
    pub good: usize,
    pub bad_after_first: usize,
    pub counted_after_first: usize,
    pub lost: u64,
    pub first_good_s: Option<f64>,
    pub good_s: Vec<f64>,
    pub offset_hz: f64,
    pub snr_db: f64,
    pub results: Results,
    pub packets: Vec<[u8; 188]>,
    pub bad_times: Vec<f64>,
    pub signal_start_s: f64,
    /// (signal time, reported offset) after TPS lock, per block fed.
    pub offsets: Vec<(f64, f64)>,
}

impl Outcome {
    /// "Good" of the spec: TPS right, more than 50 good packets, at most `one_in` bad after the first good one.
    pub fn good(&self, one_in: usize) -> bool {
        self.tps_ok
            && self.good > 50
            && self.bad_after_first * one_in <= self.counted_after_first.max(1)
    }
}

/// The signal of a case (natural rate of the generator, then the impairments), and where the signal starts in it.
pub fn signal(c: &Case) -> (Vec<Complex32>, f64, usize) {
    let mut p = GenParams::new(c.bw, c.tps);
    p.hp = Box::new(NumberedSource::new(PATTERN_HP));
    p.lp = Box::new(NumberedSource::new(PATTERN_LP));
    p.puncture_phase = c.puncture_phase;
    p.lp_rate_field = c.lp_field;
    let mut g = DvbtGenerator::new(p);
    let mut x = Vec::new();
    for _ in 0..c.frames * 68 {
        x.extend_from_slice(g.next_symbol());
    }
    let nat = c.bw.natural_rate();
    let sym = c.tps.symbol_samples();
    let skip = (c.start_frac * sym as f64) as usize;
    let mut x = x.split_off(skip);
    if c.clock_ppm != 0.0 {
        let mut ce = ClockError::new(nat, c.clock_ppm);
        let mut y = Vec::new();
        ce.process(&x, &mut y);
        x = y;
    }
    if let Some((d, db, doppler)) = c.echo {
        if doppler == 0.0 {
            Echo::new(d, db).process(&mut x);
        } else {
            let g = 10f64.powf(db / 20.0);
            let orig = x.clone();
            for n in d..x.len() {
                let a = 2.0 * std::f64::consts::PI * doppler * n as f64 / nat;
                x[n] += orig[n - d] * Complex32::new((g * a.cos()) as f32, (g * a.sin()) as f32);
            }
        }
    }
    let rate = c.input_rate.unwrap_or(nat);
    if (rate - nat).abs() > 1.0 {
        let mut r = FractionalResampler::<Complex32>::with_rates(nat, rate).unwrap();
        let mut y = Vec::new();
        r.process(&x, &mut y);
        x = y;
    }
    if c.cfo_hz != 0.0 {
        FrequencyOffset::new(rate, c.cfo_hz).process(&mut x);
    }
    let occupied = c.tps.mode.carriers() as f64 * nat / c.tps.mode.fft() as f64;
    let power = x.iter().map(|s| s.norm_sqr() as f64).sum::<f64>() / x.len() as f64;
    // Lead-in.
    let mut lead = vec![Complex32::default(); c.lead];
    if let Some(snr) = c.snr_db {
        let mut n = Awgn::new(power, snr, occupied, rate, c.seed);
        n.process(&mut x);
        n.process(&mut lead);
        if let Some((secs, bad)) = c.bad_start {
            let k = ((secs * rate) as usize).min(x.len());
            let mut b = Awgn::new(power, bad, occupied, rate, c.seed + 1);
            b.process(&mut x[..k]);
        }
        if let Some((t0, dur, snr_b)) = c.burst {
            let a = (t0 * rate) as usize;
            let b = ((t0 + dur) * rate) as usize;
            let mut nb = Awgn::new(power, snr_b, occupied, rate, c.seed + 2);
            if a < x.len() {
                let b = b.min(x.len());
                nb.process(&mut x[a..b]);
            }
        }
    }
    if let Some((dbc, ph)) = c.dc {
        let amp = (power.sqrt() * 10f64.powf(dbc / 20.0)) as f32;
        let d = Complex32::new(amp * ph.cos() as f32, amp * ph.sin() as f32);
        x.iter_mut().for_each(|s| *s += d);
        lead.iter_mut().for_each(|s| *s += d);
    }
    if let Some((gdb, deg)) = c.iq_imbalance {
        let g = 10f32.powf(gdb as f32 / 20.0);
        let p = (deg as f32).to_radians();
        for s in x.iter_mut() {
            let (i, q) = (s.re, s.im);
            *s = Complex32::new(i, g * (q * p.cos() + i * p.sin()));
        }
    }
    if c.swap_iq {
        x.iter_mut().for_each(|s| *s = Complex32::new(s.im, s.re));
        lead.iter_mut()
            .for_each(|s| *s = Complex32::new(s.im, s.re));
    }
    if let Some((n, frac)) = c.nan {
        let at = (x.len() as f64 * frac) as usize;
        for k in 0..n {
            x[at + k] = if k % 2 == 0 {
                Complex32::new(f32::NAN, 0.0)
            } else {
                Complex32::new(f32::INFINITY, -f32::INFINITY)
            };
        }
    }
    if let Some((t, n)) = c.drop {
        let at = (t * rate) as usize;
        if at + n < x.len() {
            x.drain(at..at + n);
        }
    }
    if let Some(rms) = c.clip8 {
        let cur = power.sqrt() as f32;
        let fs = cur / rms as f32;
        Quantizer::new(8, fs).process(&mut x);
    }
    let start = lead.len();
    lead.extend_from_slice(&x);
    (lead, rate, start)
}

/// Runs a case through the receiver.
pub fn run(c: &Case) -> Outcome {
    let (sig, rate, start) = signal(c);
    run_signal(c, &sig, rate, start)
}

/// The receiver of a case, configured for `rate`, with a tap on its transport stream.
pub fn receiver(c: &Case, rate: f64) -> (StackGraph, TapReader) {
    let bw = c.rx_bandwidth.unwrap_or(BandwidthChoice::of(c.bw));
    let mut settings = vec![
        ("bandwidth", choice(bw)),
        (
            "stream",
            choice(if c.lp {
                StreamChoice::Lp
            } else {
                StreamChoice::Hp
            }),
        ),
    ];
    settings.extend(c.settings.iter().cloned());
    let mut rx = stack(&settings);
    rx.configure(&InputInfo::new(rate, 600e6, 600e6).recording())
        .unwrap();
    let tap = rx.graph_mut().tap("fec.out", 1 << 14).unwrap();
    (rx, tap)
}

pub fn run_signal(c: &Case, sig: &[Complex32], rate: f64, start: usize) -> Outcome {
    let (mut rx, mut tap) = receiver(c, rate);
    let threads: Vec<_> = rx
        .staged()
        .unwrap()
        .cut(c.cuts)
        .into_iter()
        .map(|w| std::thread::spawn(move || w.run()))
        .collect();
    assert_eq!(
        threads.len(),
        c.cuts.count_ones() as usize,
        "a thread per cut"
    );
    assert_eq!(rx.staged().unwrap().cuts(), c.cuts);
    let mut buf = sdrkit_graph::Buffer::new(&sdrkit_proto_mpegts::stream::kind());
    let mut packets = Vec::new();
    let mut out = Output::default();
    let mut times = Vec::new();
    let mut fed = 0usize;
    let mut offsets = Vec::new();
    for chunk in sig.chunks(c.block) {
        rx.feed(chunk, &mut out);
        out.clear();
        fed += chunk.len();
        let r = Owner::results(&rx);
        if r.status().lock_state == LockState::Locked {
            if let Some(o) = r.fields().get(&sdrkit_core::publish::keys::OFFSET_HZ) {
                offsets.push((fed as f64 / rate, o));
            }
        }
        drain_packets(&mut tap, &mut buf, &mut packets);
        while times.len() < packets.len() {
            times.push(fed as f64 / rate);
        }
    }
    rx.staged().unwrap().drain(&mut out);
    rx.finish(&mut out);
    drop(rx.staged().unwrap().cut(0));
    for t in threads {
        t.join().unwrap();
    }
    drain_packets(&mut tap, &mut buf, &mut packets);
    while times.len() < packets.len() {
        times.push(fed as f64 / rate);
    }
    let mut o = evaluate(c, Owner::results(&rx), packets, times, start as f64 / rate);
    o.offsets = offsets;
    o
}

pub fn evaluate(
    c: &Case,
    r: &Results,
    pk: Vec<[u8; 188]>,
    times: Vec<f64>,
    start_s: f64,
) -> Outcome {
    let f = r.fields();
    let mut o = Outcome {
        results: r.clone(),
        signal_start_s: start_s,
        ..Default::default()
    };
    let t = &c.tps;
    let got = (
        f.at(&F::FFT),
        f.at(&F::GUARD),
        f.at(&F::CONSTELLATION),
        f.at(&F::HIERARCHY),
        f.at(&F::CODE_RATE_HP),
    );
    o.params = Some(format!("{got:?}"));
    o.tps_ok = got
        == (
            Some(t.mode),
            Some(t.guard),
            Some(t.constellation),
            Some(t.hierarchy),
            Some(t.rate_hp),
        );
    if t.hierarchy.hierarchical() {
        o.tps_ok &= f.at(&F::CODE_RATE_LP) == Some(t.rate_lp);
    }
    o.offset_hz = f.get(&sdrkit_core::publish::keys::OFFSET_HZ).unwrap_or(0.0);
    o.snr_db = f.get(&sdrkit_core::publish::keys::SNR_DB).unwrap_or(0.0);
    let pid = if c.lp { 0x0200 } else { 0x0100 };
    let mut last: Option<u32> = None;
    for (i, p) in pk.iter().enumerate() {
        match check_test_packet(p, pid) {
            Some(cnt) => {
                o.good += 1;
                o.good_s.push(times[i]);
                if o.first_good_s.is_none() {
                    o.first_good_s = Some(times[i]);
                }
                if let Some(l) = last {
                    if cnt > l + 1 {
                        o.lost += (cnt - l - 1) as u64;
                    }
                }
                last = Some(cnt);
                if o.first_good_s.is_some() {
                    o.counted_after_first += 1;
                }
            }
            None => {
                if o.first_good_s.is_some() {
                    o.bad_after_first += 1;
                    o.counted_after_first += 1;
                    o.bad_times.push(times[i]);
                }
            }
        }
    }
    o.packets = pk;
    o
}

/// A one-line summary for the test log.
pub fn summary(name: &str, o: &Outcome) -> String {
    format!(
        "{name}: tps {} ({}), good {}, bad {} of {}, lost {}, first good {:.3} s, offset {:.1} Hz, snr {:.1} dB",
        o.tps_ok,
        o.params.clone().unwrap_or_default(),
        o.good,
        o.bad_after_first,
        o.counted_after_first,
        o.lost,
        o.first_good_s.map_or(-1.0, |t| t - o.signal_start_s),
        o.offset_hz,
        o.snr_db
    )
}

/// The number of a numbered test packet on `pid` (the DVB-T fill), or `None` when it is not one.
pub fn check_test_packet(p: &[u8], pid: u16) -> Option<u32> {
    Numbered::new(pid, Fill::I31N7).number_of(p)
}

/// The fields of a report of `dvbt` with the detection level `detect`, the occupancy `occupancy` and no other
/// measurement: what a receiver shows before it finds a signal.
pub fn report_fields(detect: i64, occupancy: Option<f64>) -> DvbtFields<'static> {
    use sdrkit_core::interface::Hertz;
    use sdrkit_stack_dvbt::results::Quality;
    DvbtFields {
        detect_level: detect,
        channel_width: Hertz(0.0),
        bandwidth_auto: false,
        bandwidth_from: None,
        measured_width: None,
        occupancy_db: occupancy,
        input_rate: Hertz(0.0),
        natural_rate: Hertz(0.0),
        resampling: false,
        rate_ok: false,
        fft: None,
        fft_size: None,
        guard: None,
        guard_samples: None,
        carriers: None,
        constellation: None,
        hierarchy: None,
        code_rate_hp: None,
        code_rate_lp: None,
        stream: StreamChoice::Hp,
        cell_id: None,
        tps_length: None,
        dvbh_flags: None,
        frame: None,
        symbol: None,
        frame_duration: None,
        net_bitrate_bps: None,
        offset_hz: None,
        integer_offset: None,
        locator_hz: Hertz(0.0),
        clock_ppm: 0.0,
        spectrum_inverted: false,
        snr_db: None,
        mer_db: None,
        ber_pre_viterbi: 0.0,
        ber_post_viterbi: 0.0,
        viterbi_margin: 0.0,
        puncture_phase: None,
        rs_blocks: 0,
        rs_clean: 0,
        rs_corrected: 0,
        rs_failed: 0,
        rs_bytes_corrected: 0,
        packets: 0,
        fec_sync: false,
        resyncs: 0,
        since_tps: None,
        quality_pct: 0.0,
        quality_label: Quality::NoLock,
        required_cn_db: None,
        margin_db: None,
        unsupported: None,
        data_valid: false,
        first_packet: None,
        level_dbfs: None,
    }
}
